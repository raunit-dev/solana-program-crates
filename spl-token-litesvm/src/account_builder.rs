use litesvm::LiteSVM;
use solana_program_error::ProgramError;
use solana_program_pack::Pack;
use solana_sdk::{account::Account, pubkey::Pubkey};
use spl_token_2022_interface::extension::{
    confidential_transfer::ConfidentialTransferAccount,
    confidential_transfer_fee::ConfidentialTransferFeeAmount, cpi_guard::CpiGuard,
    immutable_owner::ImmutableOwner, memo_transfer::MemoTransfer,
    non_transferable::NonTransferableAccount, pausable::PausableAccount,
    transfer_fee::TransferFeeAmount, transfer_hook::TransferHookAccount, ExtensionType,
};

use crate::init_fixed_token_account_extension_data;

#[derive(Default)]
pub struct TokenAccountBuilder {
    fixed_extensions: Vec<TokenAccountExtensionState>,
}

impl TokenAccountBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_transfer_fee_amount(mut self, extension: TransferFeeAmount) -> Self {
        self.push_fixed_extension(TokenAccountExtensionState::TransferFeeAmount(extension));
        self
    }

    pub fn with_confidential_transfer_account(
        mut self,
        extension: ConfidentialTransferAccount,
    ) -> Self {
        self.push_fixed_extension(TokenAccountExtensionState::ConfidentialTransferAccount(
            Box::new(extension),
        ));
        self
    }

    pub fn with_immutable_owner(mut self) -> Self {
        self.push_fixed_extension(TokenAccountExtensionState::ImmutableOwner(ImmutableOwner));
        self
    }

    pub fn with_memo_transfer(mut self, extension: MemoTransfer) -> Self {
        self.push_fixed_extension(TokenAccountExtensionState::MemoTransfer(extension));
        self
    }

    pub fn with_cpi_guard(mut self, extension: CpiGuard) -> Self {
        self.push_fixed_extension(TokenAccountExtensionState::CpiGuard(extension));
        self
    }

    pub fn with_non_transferable_account(mut self) -> Self {
        self.push_fixed_extension(TokenAccountExtensionState::NonTransferableAccount(
            NonTransferableAccount,
        ));
        self
    }

    pub fn with_transfer_hook_account(mut self, extension: TransferHookAccount) -> Self {
        self.push_fixed_extension(TokenAccountExtensionState::TransferHookAccount(extension));
        self
    }

    pub fn with_confidential_transfer_fee_amount(
        mut self,
        extension: ConfidentialTransferFeeAmount,
    ) -> Self {
        self.push_fixed_extension(TokenAccountExtensionState::ConfidentialTransferFeeAmount(
            extension,
        ));
        self
    }

    pub fn with_pausable_account(mut self) -> Self {
        self.push_fixed_extension(TokenAccountExtensionState::PausableAccount(PausableAccount));
        self
    }

    pub fn build(
        self,
        svm: &mut LiteSVM,
        pubkey: &Pubkey,
        mint: &Pubkey,
        owner: &Pubkey,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let token_account = spl_token_2022_interface::state::Account {
            mint: *mint,
            owner: *owner,
            amount,
            delegate: None.into(),
            state: spl_token_2022_interface::state::AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let account_len = ExtensionType::try_calculate_account_len::<
            spl_token_2022_interface::state::Account,
        >(&self.extension_types())?;
        let mut data = vec![0; account_len];
        spl_token_2022_interface::state::Account::pack(
            token_account,
            &mut data[..spl_token_2022_interface::state::Account::LEN],
        )?;

        for extension in self.fixed_extensions {
            extension.init(&mut data)?;
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

    fn push_fixed_extension(&mut self, extension: TokenAccountExtensionState) {
        self.remove_fixed_extension(extension.extension_type());
        self.fixed_extensions.push(extension);
    }

    fn remove_fixed_extension(&mut self, extension_type: ExtensionType) {
        self.fixed_extensions
            .retain(|extension| extension.extension_type() != extension_type);
    }

    fn extension_types(&self) -> Vec<ExtensionType> {
        self.fixed_extensions
            .iter()
            .map(TokenAccountExtensionState::extension_type)
            .collect()
    }
}

enum TokenAccountExtensionState {
    TransferFeeAmount(TransferFeeAmount),
    ConfidentialTransferAccount(Box<ConfidentialTransferAccount>),
    ImmutableOwner(ImmutableOwner),
    MemoTransfer(MemoTransfer),
    CpiGuard(CpiGuard),
    NonTransferableAccount(NonTransferableAccount),
    TransferHookAccount(TransferHookAccount),
    ConfidentialTransferFeeAmount(ConfidentialTransferFeeAmount),
    PausableAccount(PausableAccount),
}

impl TokenAccountExtensionState {
    fn extension_type(&self) -> ExtensionType {
        match self {
            Self::TransferFeeAmount(_) => ExtensionType::TransferFeeAmount,
            Self::ConfidentialTransferAccount(_) => ExtensionType::ConfidentialTransferAccount,
            Self::ImmutableOwner(_) => ExtensionType::ImmutableOwner,
            Self::MemoTransfer(_) => ExtensionType::MemoTransfer,
            Self::CpiGuard(_) => ExtensionType::CpiGuard,
            Self::NonTransferableAccount(_) => ExtensionType::NonTransferableAccount,
            Self::TransferHookAccount(_) => ExtensionType::TransferHookAccount,
            Self::ConfidentialTransferFeeAmount(_) => ExtensionType::ConfidentialTransferFeeAmount,
            Self::PausableAccount(_) => ExtensionType::PausableAccount,
        }
    }

    fn init(self, data: &mut Vec<u8>) -> Result<(), ProgramError> {
        match self {
            Self::TransferFeeAmount(extension) => {
                init_fixed_token_account_extension_data(data, extension).map(|_| ())
            }
            Self::ConfidentialTransferAccount(extension) => {
                init_fixed_token_account_extension_data(data, *extension).map(|_| ())
            }
            Self::ImmutableOwner(extension) => {
                init_fixed_token_account_extension_data(data, extension).map(|_| ())
            }
            Self::MemoTransfer(extension) => {
                init_fixed_token_account_extension_data(data, extension).map(|_| ())
            }
            Self::CpiGuard(extension) => {
                init_fixed_token_account_extension_data(data, extension).map(|_| ())
            }
            Self::NonTransferableAccount(extension) => {
                init_fixed_token_account_extension_data(data, extension).map(|_| ())
            }
            Self::TransferHookAccount(extension) => {
                init_fixed_token_account_extension_data(data, extension).map(|_| ())
            }
            Self::ConfidentialTransferFeeAmount(extension) => {
                init_fixed_token_account_extension_data(data, extension).map(|_| ())
            }
            Self::PausableAccount(extension) => {
                init_fixed_token_account_extension_data(data, extension).map(|_| ())
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
        let token_account = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let memo_transfer = MemoTransfer {
            require_incoming_transfer_memos: true.into(),
        };

        TokenAccountBuilder::new()
            .with_immutable_owner()
            .with_memo_transfer(memo_transfer)
            .with_transfer_fee_amount(TransferFeeAmount::default())
            .build(&mut svm, &token_account, &mint, &owner, 42)
            .unwrap();

        let account = svm.get_account(&token_account).unwrap();
        assert_eq!(account.owner, spl_token_2022_interface::id());

        let expected_len = ExtensionType::try_calculate_account_len::<
            spl_token_2022_interface::state::Account,
        >(&[
            ExtensionType::ImmutableOwner,
            ExtensionType::MemoTransfer,
            ExtensionType::TransferFeeAmount,
        ])
        .unwrap();
        assert_eq!(account.data.len(), expected_len);

        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Account>::unpack(&account.data)
                .unwrap();
        assert_eq!(state.base.mint, mint);
        assert_eq!(state.base.owner, owner);
        assert_eq!(state.base.amount, 42);
        assert!(state.get_extension::<ImmutableOwner>().is_ok());
        assert!(state.get_extension::<TransferFeeAmount>().is_ok());

        let memo_transfer = state.get_extension::<MemoTransfer>().unwrap();
        assert!(bool::from(memo_transfer.require_incoming_transfer_memos));
    }

    #[test]
    fn duplicate_fixed_extension_uses_last_value() {
        let mut svm = LiteSVM::new();
        let token_account = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();

        TokenAccountBuilder::new()
            .with_memo_transfer(MemoTransfer {
                require_incoming_transfer_memos: false.into(),
            })
            .with_memo_transfer(MemoTransfer {
                require_incoming_transfer_memos: true.into(),
            })
            .build(&mut svm, &token_account, &mint, &owner, 42)
            .unwrap();

        let account = svm.get_account(&token_account).unwrap();
        let state =
            StateWithExtensions::<spl_token_2022_interface::state::Account>::unpack(&account.data)
                .unwrap();
        let memo_transfer = state.get_extension::<MemoTransfer>().unwrap();
        assert!(bool::from(memo_transfer.require_incoming_transfer_memos));
    }
}
