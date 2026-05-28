use litesvm::LiteSVM;
use solana_program_error::ProgramError;
use solana_program_pack::Pack;
use solana_sdk::{account::Account, account::AccountSharedData, pubkey, pubkey::Pubkey};
use spl_token_2022_interface::extension::{
    confidential_mint_burn::ConfidentialMintBurn,
    confidential_transfer::{ConfidentialTransferAccount, ConfidentialTransferMint},
    confidential_transfer_fee::{ConfidentialTransferFeeAmount, ConfidentialTransferFeeConfig},
    cpi_guard::CpiGuard,
    default_account_state::DefaultAccountState,
    group_member_pointer::GroupMemberPointer,
    group_pointer::GroupPointer,
    immutable_owner::ImmutableOwner,
    interest_bearing_mint::InterestBearingConfig,
    memo_transfer::MemoTransfer,
    metadata_pointer::MetadataPointer,
    mint_close_authority::MintCloseAuthority,
    non_transferable::{NonTransferable, NonTransferableAccount},
    pausable::{PausableAccount, PausableConfig},
    permanent_delegate::PermanentDelegate,
    permissioned_burn::PermissionedBurnConfig,
    scaled_ui_amount::ScaledUiAmountConfig,
    transfer_fee::{TransferFeeAmount, TransferFeeConfig},
    transfer_hook::{TransferHook, TransferHookAccount},
    BaseState, BaseStateWithExtensions, BaseStateWithExtensionsMut, Extension, StateWithExtensions,
    StateWithExtensionsMut,
};
use spl_token_group_interface::state::{TokenGroup, TokenGroupMember};
use spl_token_metadata_interface::state::TokenMetadata;
use spl_type_length_value::variable_len_pack::VariableLenPack;

pub const SPL_TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const NATIVE_MINT_ADDRESS: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

/// Directly writes an SPL token account into the SVM at the given address.
///
/// Uses Token-2022 layout which is wire-compatible with classic SPL token
/// accounts (identical `state::Account` size and field order for non-extension mints).
pub fn setup_token_account(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
    token_program: &Pubkey,
    is_native: Option<u64>,
) {
    let token_account = spl_token_2022_interface::state::Account {
        mint: *mint,
        owner: *owner,
        amount,
        delegate: None.into(),
        state: spl_token_2022_interface::state::AccountState::Initialized,
        is_native: is_native.into(),
        delegated_amount: 0,
        close_authority: None.into(),
    };

    let space = spl_token_2022_interface::state::Account::LEN;
    let rent = svm.minimum_balance_for_rent_exemption(space);
    let mut lamports = rent;
    if is_native.is_some() {
        lamports += amount;
    }

    let mut account = AccountSharedData::new(lamports, space, token_program);
    let mut data = [0u8; spl_token_2022_interface::state::Account::LEN];
    spl_token_2022_interface::state::Account::pack(token_account, &mut data).unwrap();
    account.set_data_from_slice(&data);
    svm.set_account(*pubkey, Account::from(account)).unwrap();
}

/// Directly writes an SPL token mint into the SVM at the given address.
pub fn setup_token_mint(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    decimals: u8,
    mint_authority: &Pubkey,
    token_program: &Pubkey,
) {
    let mint = spl_token_2022_interface::state::Mint {
        mint_authority: Some(*mint_authority).into(),
        supply: 0,
        decimals,
        is_initialized: true,
        freeze_authority: None.into(),
    };

    let space = spl_token_2022_interface::state::Mint::LEN;
    let rent = svm.minimum_balance_for_rent_exemption(space);
    let mut account = AccountSharedData::new(rent, space, token_program);
    let mut data = [0u8; spl_token_2022_interface::state::Mint::LEN];
    spl_token_2022_interface::state::Mint::pack(mint, &mut data).unwrap();
    account.set_data_from_slice(&data);
    svm.set_account(*pubkey, Account::from(account)).unwrap();
}

fn put_account(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    mut account: Account,
) -> Result<(), ProgramError> {
    let rent = svm.minimum_balance_for_rent_exemption(account.data.len());
    if account.lamports < rent {
        account.lamports = rent;
    }
    svm.set_account(*pubkey, account)
        .map_err(|_| ProgramError::InvalidAccountData)
}

fn append_fixed_extension<S, V>(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    extension: V,
) -> Result<(), ProgramError>
where
    S: BaseState + Pack,
    V: Extension + bytemuck::Pod + Default + Copy,
{
    let mut account = svm
        .get_account(pubkey)
        .ok_or(ProgramError::InvalidAccountData)?;
    if account.owner != spl_token_2022_interface::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let new_len = {
        let state = StateWithExtensions::<S>::unpack(&account.data)?;
        if state.get_extension_bytes::<V>().is_ok() {
            return Ok(());
        }
        state.try_get_new_account_len::<V>()?
    };

    if account.data.len() < new_len {
        account.data.resize(new_len, 0);
    }
    spl_token_2022_interface::extension::set_account_type::<S>(&mut account.data)?;

    let mut state = StateWithExtensionsMut::<S>::unpack(&mut account.data)?;
    let extension_ref = state.init_extension::<V>(false)?;
    *extension_ref = extension;

    put_account(svm, pubkey, account)
}

fn append_variable_len_mint_extension_state<V>(
    svm: &mut LiteSVM,
    pubkey: &Pubkey,
    extension: &V,
) -> Result<(), ProgramError>
where
    V: Extension + VariableLenPack,
{
    let mut account = svm
        .get_account(pubkey)
        .ok_or(ProgramError::InvalidAccountData)?;
    if account.owner != spl_token_2022_interface::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let new_len = {
        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Mint>::unpack(&account.data)?;
        if state.get_extension_bytes::<V>().is_ok() {
            return Ok(());
        }
        state.try_get_new_account_len_for_variable_len_extension(extension)?
    };

    if account.data.len() < new_len {
        account.data.resize(new_len, 0);
    }
    spl_token_2022_interface::extension::set_account_type::<spl_token_2022_interface::state::Mint>(
        &mut account.data,
    )?;

    let mut state =
        StateWithExtensionsMut::<spl_token_2022_interface::state::Mint>::unpack(&mut account.data)?;
    state.init_variable_len_extension(extension, false)?;

    put_account(svm, pubkey, account)
}

fn append_mint_extension<V>(
    svm: &mut LiteSVM,
    mint: &Pubkey,
    extension: V,
) -> Result<(), ProgramError>
where
    V: Extension + bytemuck::Pod + Default + Copy,
{
    append_fixed_extension::<spl_token_2022_interface::state::Mint, V>(svm, mint, extension)
}

fn append_variable_len_mint_extension<V>(
    svm: &mut LiteSVM,
    mint: &Pubkey,
    extension: &V,
) -> Result<(), ProgramError>
where
    V: Extension + VariableLenPack,
{
    append_variable_len_mint_extension_state::<V>(svm, mint, extension)
}

fn append_token_account_extension<V>(
    svm: &mut LiteSVM,
    token_account: &Pubkey,
    extension: V,
) -> Result<(), ProgramError>
where
    V: Extension + bytemuck::Pod + Default + Copy,
{
    append_fixed_extension::<spl_token_2022_interface::state::Account, V>(
        svm,
        token_account,
        extension,
    )
}

macro_rules! mint_extension_initializer {
    ($fn_name:ident, $extension_ty:ty) => {
        pub fn $fn_name(
            svm: &mut LiteSVM,
            mint: &Pubkey,
            extension: $extension_ty,
        ) -> Result<(), ProgramError> {
            append_mint_extension(svm, mint, extension)
        }
    };
}

macro_rules! account_extension_initializer {
    ($fn_name:ident, $extension_ty:ty) => {
        pub fn $fn_name(
            svm: &mut LiteSVM,
            token_account: &Pubkey,
            extension: $extension_ty,
        ) -> Result<(), ProgramError> {
            append_token_account_extension(svm, token_account, extension)
        }
    };
}

mint_extension_initializer!(initialize_transfer_fee_config_extension, TransferFeeConfig);
account_extension_initializer!(initialize_transfer_fee_amount_extension, TransferFeeAmount);
mint_extension_initializer!(
    initialize_mint_close_authority_extension,
    MintCloseAuthority
);
mint_extension_initializer!(
    initialize_confidential_transfer_mint_extension,
    ConfidentialTransferMint
);
account_extension_initializer!(
    initialize_confidential_transfer_account_extension,
    ConfidentialTransferAccount
);
mint_extension_initializer!(
    initialize_default_account_state_extension,
    DefaultAccountState
);
account_extension_initializer!(initialize_immutable_owner_extension, ImmutableOwner);
account_extension_initializer!(initialize_memo_transfer_extension, MemoTransfer);
mint_extension_initializer!(initialize_non_transferable_extension, NonTransferable);
mint_extension_initializer!(
    initialize_interest_bearing_config_extension,
    InterestBearingConfig
);
account_extension_initializer!(initialize_cpi_guard_extension, CpiGuard);
mint_extension_initializer!(initialize_permanent_delegate_extension, PermanentDelegate);
account_extension_initializer!(
    initialize_non_transferable_account_extension,
    NonTransferableAccount
);
mint_extension_initializer!(initialize_transfer_hook_extension, TransferHook);
account_extension_initializer!(
    initialize_transfer_hook_account_extension,
    TransferHookAccount
);
mint_extension_initializer!(
    initialize_confidential_transfer_fee_config_extension,
    ConfidentialTransferFeeConfig
);
account_extension_initializer!(
    initialize_confidential_transfer_fee_amount_extension,
    ConfidentialTransferFeeAmount
);
mint_extension_initializer!(initialize_metadata_pointer_extension, MetadataPointer);
mint_extension_initializer!(initialize_group_pointer_extension, GroupPointer);
mint_extension_initializer!(initialize_token_group_extension, TokenGroup);
mint_extension_initializer!(
    initialize_group_member_pointer_extension,
    GroupMemberPointer
);
mint_extension_initializer!(initialize_token_group_member_extension, TokenGroupMember);
mint_extension_initializer!(
    initialize_confidential_mint_burn_extension,
    ConfidentialMintBurn
);
mint_extension_initializer!(
    initialize_scaled_ui_amount_config_extension,
    ScaledUiAmountConfig
);
mint_extension_initializer!(initialize_pausable_config_extension, PausableConfig);
account_extension_initializer!(initialize_pausable_account_extension, PausableAccount);
mint_extension_initializer!(
    initialize_permissioned_burn_config_extension,
    PermissionedBurnConfig
);

pub fn initialize_token_metadata_extension(
    svm: &mut LiteSVM,
    mint: &Pubkey,
    extension: &TokenMetadata,
) -> Result<(), ProgramError> {
    append_variable_len_mint_extension(svm, mint, extension)
}

/// Adds tokens to an existing token account by directly mutating its state.
pub fn add_tokens_to_token_account(svm: &mut LiteSVM, token_account_pubkey: &Pubkey, amount: u64) {
    let mut token_account_data = svm.get_account(token_account_pubkey).unwrap().data.clone();
    let mut token_account =
        spl_token_2022_interface::state::Account::unpack_from_slice(&token_account_data).unwrap();

    token_account.amount = token_account
        .amount
        .checked_add(amount)
        .expect("overflow adding tokens to token account");

    spl_token_2022_interface::state::Account::pack(
        token_account,
        &mut token_account_data[..spl_token_2022_interface::state::Account::LEN],
    )
    .unwrap();

    let mut account = svm.get_account(token_account_pubkey).unwrap();
    account.data = token_account_data;
    svm.set_account(*token_account_pubkey, account).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use spl_token_2022_interface::extension::ExtensionType;

    #[test]
    fn initializes_mint_extension_once() {
        let mut svm = LiteSVM::new();
        let token_program = spl_token_2022_interface::id();
        let mint = Pubkey::new_unique();
        let authority = Pubkey::new_unique();

        setup_token_mint(&mut svm, &mint, 6, &authority, &token_program);
        assert_eq!(
            svm.get_account(&mint).unwrap().data.len(),
            spl_token_2022_interface::state::Mint::LEN
        );

        initialize_mint_close_authority_extension(&mut svm, &mint, MintCloseAuthority::default())
            .unwrap();

        let account = svm.get_account(&mint).unwrap();
        let expected_len = ExtensionType::try_calculate_account_len::<
            spl_token_2022_interface::state::Mint,
        >(&[ExtensionType::MintCloseAuthority])
        .unwrap();
        assert_eq!(account.data.len(), expected_len);
        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Mint>::unpack(&account.data)
                .unwrap();
        assert!(state.get_extension::<MintCloseAuthority>().is_ok());

        initialize_mint_close_authority_extension(&mut svm, &mint, MintCloseAuthority::default())
            .unwrap();
        assert_eq!(svm.get_account(&mint).unwrap().data.len(), expected_len);
    }

    #[test]
    fn initializes_token_account_extension_once() {
        let mut svm = LiteSVM::new();
        let token_program = spl_token_2022_interface::id();
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let token_account = Pubkey::new_unique();

        setup_token_account(
            &mut svm,
            &token_account,
            &mint,
            &owner,
            42,
            &token_program,
            None,
        );

        initialize_immutable_owner_extension(&mut svm, &token_account, ImmutableOwner).unwrap();

        let account = svm.get_account(&token_account).unwrap();
        let expected_len = ExtensionType::try_calculate_account_len::<
            spl_token_2022_interface::state::Account,
        >(&[ExtensionType::ImmutableOwner])
        .unwrap();
        assert_eq!(account.data.len(), expected_len);
        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Account>::unpack(&account.data)
                .unwrap();
        assert!(state.get_extension::<ImmutableOwner>().is_ok());

        initialize_immutable_owner_extension(&mut svm, &token_account, ImmutableOwner).unwrap();
        assert_eq!(
            svm.get_account(&token_account).unwrap().data.len(),
            expected_len
        );
    }

    #[test]
    fn add_tokens_preserves_token_account_extensions() {
        let mut svm = LiteSVM::new();
        let token_program = spl_token_2022_interface::id();
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let token_account = Pubkey::new_unique();

        setup_token_account(
            &mut svm,
            &token_account,
            &mint,
            &owner,
            42,
            &token_program,
            None,
        );
        initialize_immutable_owner_extension(&mut svm, &token_account, ImmutableOwner).unwrap();

        let len = svm.get_account(&token_account).unwrap().data.len();
        add_tokens_to_token_account(&mut svm, &token_account, 8);

        let account = svm.get_account(&token_account).unwrap();
        assert_eq!(account.data.len(), len);
        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Account>::unpack(&account.data)
                .unwrap();
        assert_eq!(state.base.amount, 50);
        assert!(state.get_extension::<ImmutableOwner>().is_ok());
    }

    #[test]
    fn initializes_variable_len_mint_extension_once() {
        let mut svm = LiteSVM::new();
        let token_program = spl_token_2022_interface::id();
        let mint = Pubkey::new_unique();
        let authority = Pubkey::new_unique();

        setup_token_mint(&mut svm, &mint, 9, &authority, &token_program);
        let metadata = TokenMetadata {
            mint,
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com/test.json".to_string(),
            ..Default::default()
        };

        initialize_token_metadata_extension(&mut svm, &mint, &metadata).unwrap();

        let account = svm.get_account(&mint).unwrap();
        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Mint>::unpack(&account.data)
                .unwrap();
        let extension = state.get_variable_len_extension::<TokenMetadata>().unwrap();
        assert_eq!(extension.name, "Test Token");
        let len = account.data.len();

        let replacement = TokenMetadata {
            name: "Replacement".to_string(),
            ..metadata
        };
        initialize_token_metadata_extension(&mut svm, &mint, &replacement).unwrap();

        let account = svm.get_account(&mint).unwrap();
        assert_eq!(account.data.len(), len);
        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Mint>::unpack(&account.data)
                .unwrap();
        let extension = state.get_variable_len_extension::<TokenMetadata>().unwrap();
        assert_eq!(extension.name, "Test Token");
    }
}
