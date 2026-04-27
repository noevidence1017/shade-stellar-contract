use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotAuthorized = 3,
    InsufficientBalance = 4,
    AccountRestricted = 5,
    InvoiceNotFound = 6,
    InvalidInvoiceStatus = 7,
}
