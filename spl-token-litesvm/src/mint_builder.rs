use litesvm::LiteSVM;
use solana_program_error::ProgramError;
use solana_program_pack::Pack;
use solana_sdk::{account::Account, pubkey::Pubkey};
use spl_token_2022_interface::extension::{
    confidential_mint_burn::ConfidentialMintBurn, confidential_transfer::ConfidentialTransferMint,
    confidential_transfer_fee::ConfidentialTransferFeeConfig,
    default_account_state::DefaultAccountState, group_member_pointer::GroupMemberPointer,
    group_pointer::GroupPointer, interest_bearing_mint::InterestBearingConfig,
    metadata_pointer::MetadataPointer, mint_close_authority::MintCloseAuthority,
    non_transferable::NonTransferable, pausable::PausableConfig,
    permanent_delegate::PermanentDelegate, permissioned_burn::PermissionedBurnConfig,
    scaled_ui_amount::ScaledUiAmountConfig, transfer_fee::TransferFeeConfig,
    transfer_hook::TransferHook, ExtensionType,
};
use spl_token_group_interface::state::{TokenGroup, TokenGroupMember};
use spl_token_metadata_interface::state::TokenMetadata;
use spl_type_length_value::variable_len_pack::VariableLenPack;
use std::mem::size_of;

use crate::{init_fixed_mint_extension_data, init_variable_len_mint_extension_data};

#[derive(Default)]
pub struct MintBuilder {
    fixed_extensions: Vec<MintExtensionState>,
    inline_metadata: Option<(TokenMetadata, Pubkey)>,
}

impl MintBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_transfer_fee(mut self, extension: TransferFeeConfig) -> Self {
        self.push_fixed_extension(MintExtensionState::TransferFeeConfig(extension));
        self
    }

    pub fn with_mint_close_authority(mut self, extension: MintCloseAuthority) -> Self {
        self.push_fixed_extension(MintExtensionState::MintCloseAuthority(extension));
        self
    }

    pub fn with_confidential_transfer_mint(mut self, extension: ConfidentialTransferMint) -> Self {
        self.push_fixed_extension(MintExtensionState::ConfidentialTransferMint(extension));
        self
    }

    pub fn with_default_account_state(mut self, extension: DefaultAccountState) -> Self {
        self.push_fixed_extension(MintExtensionState::DefaultAccountState(extension));
        self
    }

    pub fn with_non_transferable(mut self, extension: NonTransferable) -> Self {
        self.push_fixed_extension(MintExtensionState::NonTransferable(extension));
        self
    }

    pub fn with_interest_bearing_config(mut self, extension: InterestBearingConfig) -> Self {
        self.push_fixed_extension(MintExtensionState::InterestBearingConfig(extension));
        self
    }

    pub fn with_permanent_delegate(mut self, extension: PermanentDelegate) -> Self {
        self.push_fixed_extension(MintExtensionState::PermanentDelegate(extension));
        self
    }

    pub fn with_transfer_hook(mut self, extension: TransferHook) -> Self {
        self.push_fixed_extension(MintExtensionState::TransferHook(extension));
        self
    }

    pub fn with_confidential_transfer_fee_config(
        mut self,
        extension: ConfidentialTransferFeeConfig,
    ) -> Self {
        self.push_fixed_extension(MintExtensionState::ConfidentialTransferFeeConfig(extension));
        self
    }

    pub fn with_metadata_pointer(mut self, extension: MetadataPointer) -> Self {
        self.inline_metadata = None;
        self.push_fixed_extension(MintExtensionState::MetadataPointer(extension));
        self
    }

    pub fn with_group_pointer(mut self, extension: GroupPointer) -> Self {
        self.push_fixed_extension(MintExtensionState::GroupPointer(extension));
        self
    }

    pub fn with_token_group(mut self, extension: TokenGroup) -> Self {
        self.push_fixed_extension(MintExtensionState::TokenGroup(extension));
        self
    }

    pub fn with_group_member_pointer(mut self, extension: GroupMemberPointer) -> Self {
        self.push_fixed_extension(MintExtensionState::GroupMemberPointer(extension));
        self
    }

    pub fn with_token_group_member(mut self, extension: TokenGroupMember) -> Self {
        self.push_fixed_extension(MintExtensionState::TokenGroupMember(extension));
        self
    }

    pub fn with_confidential_mint_burn(mut self, extension: ConfidentialMintBurn) -> Self {
        self.push_fixed_extension(MintExtensionState::ConfidentialMintBurn(extension));
        self
    }

    pub fn with_scaled_ui_amount_config(mut self, extension: ScaledUiAmountConfig) -> Self {
        self.push_fixed_extension(MintExtensionState::ScaledUiAmountConfig(extension));
        self
    }

    pub fn with_pausable_config(mut self, extension: PausableConfig) -> Self {
        self.push_fixed_extension(MintExtensionState::PausableConfig(extension));
        self
    }

    pub fn with_permissioned_burn_config(mut self, extension: PermissionedBurnConfig) -> Self {
        self.push_fixed_extension(MintExtensionState::PermissionedBurnConfig(extension));
        self
    }

    pub fn with_inline_metadata(
        mut self,
        extension: TokenMetadata,
        metadata_authority: &Pubkey,
    ) -> Self {
        self.remove_fixed_extension(ExtensionType::MetadataPointer);
        self.inline_metadata = Some((extension, *metadata_authority));
        self
    }

    pub fn build(
        self,
        svm: &mut LiteSVM,
        pubkey: &Pubkey,
        decimals: u8,
        mint_authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        let mint = spl_token_2022_interface::state::Mint {
            mint_authority: Some(*mint_authority).into(),
            supply: 0,
            decimals,
            is_initialized: true,
            freeze_authority: None.into(),
        };
        let mint_len = self.mint_len()?;
        let mut data = vec![0; mint_len];
        spl_token_2022_interface::state::Mint::pack(
            mint,
            &mut data[..spl_token_2022_interface::state::Mint::LEN],
        )?;

        for extension in self.fixed_extensions {
            extension.init(&mut data)?;
        }

        if let Some((mut metadata, metadata_authority)) = self.inline_metadata {
            let metadata_pointer = MetadataPointer {
                authority: metadata_authority.into(),
                metadata_address: (*pubkey).into(),
            };
            init_fixed_mint_extension_data(&mut data, metadata_pointer)?;
            metadata.update_authority = metadata_authority.into();
            metadata.mint = *pubkey;
            init_variable_len_mint_extension_data(&mut data, &metadata)?;
        }

        let account = Account {
            lamports: svm.minimum_balance_for_rent_exemption(data.len()),
            data,
            owner: spl_token_2022_interface::id(),
            executable: false,
            rent_epoch: 0,
        };
        svm.set_account(*pubkey, account)
            .map_err(|_| ProgramError::InvalidAccountData)
    }

    fn push_fixed_extension(&mut self, extension: MintExtensionState) {
        self.remove_fixed_extension(extension.extension_type());
        self.fixed_extensions.push(extension);
    }

    fn remove_fixed_extension(&mut self, extension_type: ExtensionType) {
        self.fixed_extensions
            .retain(|extension| extension.extension_type() != extension_type);
    }

    fn extension_types(&self) -> Vec<ExtensionType> {
        let mut extension_types: Vec<_> = self
            .fixed_extensions
            .iter()
            .map(MintExtensionState::extension_type)
            .collect();
        if self.inline_metadata.is_some()
            && !extension_types.contains(&ExtensionType::MetadataPointer)
        {
            extension_types.push(ExtensionType::MetadataPointer);
        }
        extension_types
    }

    fn mint_len(&self) -> Result<usize, ProgramError> {
        let extension_types = self.extension_types();
        let mut mint_len = ExtensionType::try_calculate_account_len::<
            spl_token_2022_interface::state::Mint,
        >(&extension_types)?;

        if let Some(metadata) = self.variable_metadata() {
            if extension_types.is_empty() {
                mint_len = spl_token_2022_interface::state::Mint::LEN + size_of::<u8>();
            }
            // TLV Header (Type: u16, Length: u16) = 4 bytes
            mint_len = mint_len
                .saturating_add(4)
                .saturating_add(metadata.get_packed_len()?);
        }

        Ok(mint_len)
    }

    fn variable_metadata(&self) -> Option<&TokenMetadata> {
        self.inline_metadata.as_ref().map(|(metadata, _)| metadata)
    }
}

enum MintExtensionState {
    TransferFeeConfig(TransferFeeConfig),
    MintCloseAuthority(MintCloseAuthority),
    ConfidentialTransferMint(ConfidentialTransferMint),
    DefaultAccountState(DefaultAccountState),
    NonTransferable(NonTransferable),
    InterestBearingConfig(InterestBearingConfig),
    PermanentDelegate(PermanentDelegate),
    TransferHook(TransferHook),
    ConfidentialTransferFeeConfig(ConfidentialTransferFeeConfig),
    MetadataPointer(MetadataPointer),
    GroupPointer(GroupPointer),
    TokenGroup(TokenGroup),
    GroupMemberPointer(GroupMemberPointer),
    TokenGroupMember(TokenGroupMember),
    ConfidentialMintBurn(ConfidentialMintBurn),
    ScaledUiAmountConfig(ScaledUiAmountConfig),
    PausableConfig(PausableConfig),
    PermissionedBurnConfig(PermissionedBurnConfig),
}

impl MintExtensionState {
    fn extension_type(&self) -> ExtensionType {
        match self {
            Self::TransferFeeConfig(_) => ExtensionType::TransferFeeConfig,
            Self::MintCloseAuthority(_) => ExtensionType::MintCloseAuthority,
            Self::ConfidentialTransferMint(_) => ExtensionType::ConfidentialTransferMint,
            Self::DefaultAccountState(_) => ExtensionType::DefaultAccountState,
            Self::NonTransferable(_) => ExtensionType::NonTransferable,
            Self::InterestBearingConfig(_) => ExtensionType::InterestBearingConfig,
            Self::PermanentDelegate(_) => ExtensionType::PermanentDelegate,
            Self::TransferHook(_) => ExtensionType::TransferHook,
            Self::ConfidentialTransferFeeConfig(_) => ExtensionType::ConfidentialTransferFeeConfig,
            Self::MetadataPointer(_) => ExtensionType::MetadataPointer,
            Self::GroupPointer(_) => ExtensionType::GroupPointer,
            Self::TokenGroup(_) => ExtensionType::TokenGroup,
            Self::GroupMemberPointer(_) => ExtensionType::GroupMemberPointer,
            Self::TokenGroupMember(_) => ExtensionType::TokenGroupMember,
            Self::ConfidentialMintBurn(_) => ExtensionType::ConfidentialMintBurn,
            Self::ScaledUiAmountConfig(_) => ExtensionType::ScaledUiAmount,
            Self::PausableConfig(_) => ExtensionType::Pausable,
            Self::PermissionedBurnConfig(_) => ExtensionType::PermissionedBurn,
        }
    }

    fn init(self, data: &mut Vec<u8>) -> Result<(), ProgramError> {
        match self {
            Self::TransferFeeConfig(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::MintCloseAuthority(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::ConfidentialTransferMint(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::DefaultAccountState(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::NonTransferable(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::InterestBearingConfig(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::PermanentDelegate(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::TransferHook(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::ConfidentialTransferFeeConfig(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::MetadataPointer(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::GroupPointer(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::TokenGroup(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::GroupMemberPointer(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::TokenGroupMember(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::ConfidentialMintBurn(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::ScaledUiAmountConfig(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::PausableConfig(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
            Self::PermissionedBurnConfig(extension) => {
                init_fixed_mint_extension_data(data, extension).map(|_| ())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spl_token_2022_interface::extension::{BaseStateWithExtensions, StateWithExtensions};

    #[test]
    fn initializes_multiple_fixed_extensions() {
        let mut svm = LiteSVM::new();
        let mint = Pubkey::new_unique();
        let authority = Pubkey::new_unique();

        MintBuilder::new()
            .with_mint_close_authority(MintCloseAuthority::default())
            .with_permanent_delegate(PermanentDelegate::default())
            .build(&mut svm, &mint, 6, &authority)
            .unwrap();

        let account = svm.get_account(&mint).unwrap();
        assert_eq!(account.owner, spl_token_2022_interface::id());

        let expected_len =
            ExtensionType::try_calculate_account_len::<spl_token_2022_interface::state::Mint>(&[
                ExtensionType::MintCloseAuthority,
                ExtensionType::PermanentDelegate,
            ])
            .unwrap();
        assert_eq!(account.data.len(), expected_len);

        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Mint>::unpack(&account.data)
                .unwrap();
        assert_eq!(state.base.decimals, 6);
        assert!(state.get_extension::<MintCloseAuthority>().is_ok());
        assert!(state.get_extension::<PermanentDelegate>().is_ok());
    }

    #[test]
    fn initializes_inline_metadata() {
        let mut svm = LiteSVM::new();
        let mint = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let metadata_authority = Pubkey::new_unique();
        let metadata = TokenMetadata {
            name: "Builder Token".to_string(),
            symbol: "BLDR".to_string(),
            uri: "https://example.com/builder.json".to_string(),
            ..Default::default()
        };

        MintBuilder::new()
            .with_inline_metadata(metadata, &metadata_authority)
            .build(&mut svm, &mint, 9, &authority)
            .unwrap();

        let account = svm.get_account(&mint).unwrap();
        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Mint>::unpack(&account.data)
                .unwrap();
        let pointer = state.get_extension::<MetadataPointer>().unwrap();
        assert_eq!(pointer.authority, metadata_authority.into());
        assert_eq!(pointer.metadata_address, mint.into());

        let metadata = state.get_variable_len_extension::<TokenMetadata>().unwrap();
        assert_eq!(metadata.name, "Builder Token");
        assert_eq!(metadata.symbol, "BLDR");
        assert_eq!(metadata.uri, "https://example.com/builder.json");
        assert_eq!(metadata.mint, mint);
        assert_eq!(metadata.update_authority, metadata_authority.into());
    }

    #[test]
    fn rejects_invalid_extension_combination() {
        let mut svm = LiteSVM::new();
        let mint = Pubkey::new_unique();
        let authority = Pubkey::new_unique();

        let err = MintBuilder::new()
            .with_scaled_ui_amount_config(ScaledUiAmountConfig::default())
            .with_interest_bearing_config(InterestBearingConfig::default())
            .build(&mut svm, &mint, 6, &authority)
            .unwrap_err();

        assert_eq!(
            err,
            ProgramError::from(
                spl_token_2022_interface::error::TokenError::InvalidExtensionCombination
            )
        );
        assert!(svm.get_account(&mint).is_none());
    }

    #[test]
    fn duplicate_fixed_extension_uses_last_value() {
        let mut svm = LiteSVM::new();
        let mint = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let first_authority = Pubkey::new_unique();
        let second_authority = Pubkey::new_unique();

        MintBuilder::new()
            .with_mint_close_authority(MintCloseAuthority {
                close_authority: first_authority.into(),
            })
            .with_mint_close_authority(MintCloseAuthority {
                close_authority: second_authority.into(),
            })
            .build(&mut svm, &mint, 6, &authority)
            .unwrap();

        let account = svm.get_account(&mint).unwrap();
        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Mint>::unpack(&account.data)
                .unwrap();
        let close_authority = state.get_extension::<MintCloseAuthority>().unwrap();
        assert_eq!(close_authority.close_authority, second_authority.into());
    }

    #[test]
    fn initializes_external_metadata_pointer() {
        let mut svm = LiteSVM::new();
        let mint = Pubkey::new_unique();
        let external_metadata_addr = Pubkey::new_unique();
        let authority = Pubkey::new_unique();

        MintBuilder::new()
            .with_metadata_pointer(MetadataPointer {
                authority: authority.into(),
                metadata_address: external_metadata_addr.into(),
            })
            .build(&mut svm, &mint, 6, &authority)
            .unwrap();

        let account = svm.get_account(&mint).unwrap();
        // Length should only be Base + AccountType + Pointer (no variable data)
        let expected_len = ExtensionType::try_calculate_account_len::<
            spl_token_2022_interface::state::Mint,
        >(&[ExtensionType::MetadataPointer])
        .unwrap();
        assert_eq!(account.data.len(), expected_len);

        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Mint>::unpack(&account.data)
                .unwrap();
        let pointer = state.get_extension::<MetadataPointer>().unwrap();
        assert_eq!(pointer.metadata_address, external_metadata_addr.into());
    }
}
