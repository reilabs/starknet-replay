use starknet_api::core::{
    ClassHash,
    CompiledClassHash,
    ContractAddress,
    EntryPointSelector,
    Nonce,
};
use starknet_api::transaction::{
    AccountDeploymentData,
    Calldata,
    ContractAddressSalt,
    DeclareTransaction,
    DeclareTransactionV0V1,
    DeclareTransactionV2,
    DeclareTransactionV3,
    DeployAccountTransaction,
    DeployAccountTransactionV1,
    DeployAccountTransactionV3,
    DeployTransaction,
    Fee,
    InvokeTransaction,
    InvokeTransactionV0,
    InvokeTransactionV1,
    InvokeTransactionV3,
    L1HandlerTransaction,
    PaymasterData,
    Resource,
    ResourceBounds,
    Tip,
    Transaction as StarknetApiTransaction,
    TransactionSignature,
    TransactionVersion,
};
use starknet_core::types::Transaction as StarknetCoreTransaction;

use crate::error::DatabaseError;

/// This function converts [`starknet_core::types::Transaction`] into
/// [`starknet_api::transaction::Transaction`].
///
/// # Arguments
///
/// - `transaction`: The transaction object.
///
/// # Errors
///
/// Returns [`Err`] if `transaction` contains invalid numbers that can't be
/// translated to a [`starknet_api::transaction::Transaction`] object.
#[allow(clippy::too_many_lines)] // Added because there is a lot of repetition.
pub fn convert_transaction(
    transaction: StarknetCoreTransaction,
) -> Result<StarknetApiTransaction, DatabaseError> {
    match transaction {
        starknet_core::types::Transaction::Invoke(tx) => match tx {
            starknet_core::types::InvokeTransaction::V0(tx) => {
                let invoke_tx = InvokeTransactionV0 {
                    max_fee: Fee(tx.max_fee.to_string().parse()?),
                    signature: TransactionSignature(tx.signature),
                    contract_address: ContractAddress(tx.contract_address.try_into()?),
                    entry_point_selector: EntryPointSelector(tx.entry_point_selector),
                    calldata: Calldata(tx.calldata.into()),
                };
                Ok(StarknetApiTransaction::Invoke(InvokeTransaction::V0(
                    invoke_tx,
                )))
            }
            starknet_core::types::InvokeTransaction::V1(tx) => {
                let invoke_tx = InvokeTransactionV1 {
                    max_fee: Fee(tx.max_fee.to_string().parse()?),
                    signature: TransactionSignature(tx.signature),
                    nonce: Nonce(tx.nonce),
                    sender_address: ContractAddress(tx.sender_address.try_into()?),
                    calldata: Calldata(tx.calldata.into()),
                };
                Ok(StarknetApiTransaction::Invoke(InvokeTransaction::V1(
                    invoke_tx,
                )))
            }
            starknet_core::types::InvokeTransaction::V3(tx) => {
                let mut resource_bounds: Vec<(Resource, ResourceBounds)> = Vec::new();
                let l1_resource = (
                    Resource::L1Gas,
                    ResourceBounds {
                        max_amount: tx.resource_bounds.l1_gas.max_amount,
                        max_price_per_unit: tx.resource_bounds.l1_gas.max_price_per_unit,
                    },
                );
                let l2_resource = (
                    Resource::L2Gas,
                    ResourceBounds {
                        max_amount: tx.resource_bounds.l2_gas.max_amount,
                        max_price_per_unit: tx.resource_bounds.l2_gas.max_price_per_unit,
                    },
                );
                resource_bounds.push(l1_resource);
                resource_bounds.push(l2_resource);
                let invoke_tx = InvokeTransactionV3 {
                    resource_bounds: resource_bounds.try_into()?,
                    tip: Tip(tx.tip),
                    signature: TransactionSignature(tx.signature),
                    nonce: Nonce(tx.nonce),
                    sender_address: ContractAddress(tx.sender_address.try_into()?),
                    calldata: Calldata(tx.calldata.into()),
                    nonce_data_availability_mode: match tx.nonce_data_availability_mode {
                        starknet_core::types::DataAvailabilityMode::L1 => {
                            starknet_api::data_availability::DataAvailabilityMode::L1
                        }
                        starknet_core::types::DataAvailabilityMode::L2 => {
                            starknet_api::data_availability::DataAvailabilityMode::L2
                        }
                    },
                    fee_data_availability_mode: match tx.fee_data_availability_mode {
                        starknet_core::types::DataAvailabilityMode::L1 => {
                            starknet_api::data_availability::DataAvailabilityMode::L1
                        }
                        starknet_core::types::DataAvailabilityMode::L2 => {
                            starknet_api::data_availability::DataAvailabilityMode::L2
                        }
                    },
                    paymaster_data: PaymasterData(tx.paymaster_data),
                    account_deployment_data: AccountDeploymentData(tx.account_deployment_data),
                };
                Ok(StarknetApiTransaction::Invoke(InvokeTransaction::V3(
                    invoke_tx,
                )))
            }
        },
        starknet_core::types::Transaction::L1Handler(tx) => {
            let l1handler_tx = L1HandlerTransaction {
                version: TransactionVersion(tx.version),
                nonce: Nonce(tx.nonce.into()),
                contract_address: ContractAddress(tx.contract_address.try_into()?),
                entry_point_selector: EntryPointSelector(tx.entry_point_selector),
                calldata: Calldata(tx.calldata.into()),
            };
            Ok(StarknetApiTransaction::L1Handler(l1handler_tx))
        }
        starknet_core::types::Transaction::Declare(tx) => match tx {
            starknet_core::types::DeclareTransaction::V0(tx) => {
                let declare_tx = DeclareTransactionV0V1 {
                    max_fee: Fee(tx.max_fee.to_string().parse()?),
                    signature: TransactionSignature(tx.signature),
                    nonce: Nonce::default(), // Starts at 0
                    class_hash: ClassHash(tx.class_hash),
                    sender_address: ContractAddress(tx.sender_address.try_into()?),
                };
                Ok(StarknetApiTransaction::Declare(DeclareTransaction::V0(
                    declare_tx,
                )))
            }
            starknet_core::types::DeclareTransaction::V1(tx) => {
                let declare_tx = DeclareTransactionV0V1 {
                    max_fee: Fee(tx.max_fee.to_string().parse()?),
                    signature: TransactionSignature(tx.signature),
                    nonce: Nonce(tx.nonce),
                    class_hash: ClassHash(tx.class_hash),
                    sender_address: ContractAddress(tx.sender_address.try_into()?),
                };
                Ok(StarknetApiTransaction::Declare(DeclareTransaction::V0(
                    declare_tx,
                )))
            }
            starknet_core::types::DeclareTransaction::V2(tx) => {
                let declare_tx = DeclareTransactionV2 {
                    max_fee: Fee(tx.max_fee.to_string().parse()?),
                    signature: TransactionSignature(tx.signature),
                    nonce: Nonce(tx.nonce),
                    class_hash: ClassHash(tx.class_hash),
                    compiled_class_hash: CompiledClassHash(tx.compiled_class_hash),
                    sender_address: ContractAddress(tx.sender_address.try_into()?),
                };
                Ok(StarknetApiTransaction::Declare(DeclareTransaction::V2(
                    declare_tx,
                )))
            }
            starknet_core::types::DeclareTransaction::V3(tx) => {
                let mut resource_bounds: Vec<(Resource, ResourceBounds)> = Vec::new();
                let l1_resource = (
                    Resource::L1Gas,
                    ResourceBounds {
                        max_amount: tx.resource_bounds.l1_gas.max_amount,
                        max_price_per_unit: tx.resource_bounds.l1_gas.max_price_per_unit,
                    },
                );
                let l2_resource = (
                    Resource::L2Gas,
                    ResourceBounds {
                        max_amount: tx.resource_bounds.l2_gas.max_amount,
                        max_price_per_unit: tx.resource_bounds.l2_gas.max_price_per_unit,
                    },
                );
                resource_bounds.push(l1_resource);
                resource_bounds.push(l2_resource);
                let declare_tx = DeclareTransactionV3 {
                    resource_bounds: resource_bounds.try_into()?,
                    tip: Tip(tx.tip),
                    signature: TransactionSignature(tx.signature),
                    nonce: Nonce(tx.nonce),
                    class_hash: ClassHash(tx.class_hash),
                    compiled_class_hash: CompiledClassHash(tx.compiled_class_hash),
                    sender_address: ContractAddress(tx.sender_address.try_into()?),
                    nonce_data_availability_mode: match tx.nonce_data_availability_mode {
                        starknet_core::types::DataAvailabilityMode::L1 => {
                            starknet_api::data_availability::DataAvailabilityMode::L1
                        }
                        starknet_core::types::DataAvailabilityMode::L2 => {
                            starknet_api::data_availability::DataAvailabilityMode::L2
                        }
                    },
                    fee_data_availability_mode: match tx.fee_data_availability_mode {
                        starknet_core::types::DataAvailabilityMode::L1 => {
                            starknet_api::data_availability::DataAvailabilityMode::L1
                        }
                        starknet_core::types::DataAvailabilityMode::L2 => {
                            starknet_api::data_availability::DataAvailabilityMode::L2
                        }
                    },
                    paymaster_data: PaymasterData(tx.paymaster_data),
                    account_deployment_data: AccountDeploymentData(tx.account_deployment_data),
                };
                Ok(StarknetApiTransaction::Declare(DeclareTransaction::V3(
                    declare_tx,
                )))
            }
        },
        starknet_core::types::Transaction::Deploy(tx) => {
            let deploy_tx = DeployTransaction {
                version: TransactionVersion(tx.version),
                class_hash: ClassHash(tx.class_hash),
                contract_address_salt: ContractAddressSalt(tx.contract_address_salt),
                constructor_calldata: Calldata(tx.constructor_calldata.into()),
            };
            Ok(StarknetApiTransaction::Deploy(deploy_tx))
        }
        starknet_core::types::Transaction::DeployAccount(tx) => match tx {
            starknet_core::types::DeployAccountTransaction::V1(tx) => {
                let deploy_account_tx = DeployAccountTransactionV1 {
                    max_fee: Fee(tx.max_fee.to_string().parse()?),
                    signature: TransactionSignature(tx.signature),
                    nonce: Nonce(tx.nonce),
                    class_hash: ClassHash(tx.class_hash),
                    contract_address_salt: ContractAddressSalt(tx.contract_address_salt),
                    constructor_calldata: Calldata(tx.constructor_calldata.into()),
                };
                Ok(StarknetApiTransaction::DeployAccount(
                    DeployAccountTransaction::V1(deploy_account_tx),
                ))
            }
            starknet_core::types::DeployAccountTransaction::V3(tx) => {
                let mut resource_bounds: Vec<(Resource, ResourceBounds)> = Vec::new();
                let l1_resource = (
                    Resource::L1Gas,
                    ResourceBounds {
                        max_amount: tx.resource_bounds.l1_gas.max_amount,
                        max_price_per_unit: tx.resource_bounds.l1_gas.max_price_per_unit,
                    },
                );
                let l2_resource = (
                    Resource::L2Gas,
                    ResourceBounds {
                        max_amount: tx.resource_bounds.l2_gas.max_amount,
                        max_price_per_unit: tx.resource_bounds.l2_gas.max_price_per_unit,
                    },
                );
                resource_bounds.push(l1_resource);
                resource_bounds.push(l2_resource);
                let deploy_account_tx = DeployAccountTransactionV3 {
                    resource_bounds: resource_bounds.try_into()?,
                    tip: Tip(tx.tip),
                    signature: TransactionSignature(tx.signature),
                    nonce: Nonce(tx.nonce),
                    class_hash: ClassHash(tx.class_hash),
                    contract_address_salt: ContractAddressSalt(tx.contract_address_salt),
                    constructor_calldata: Calldata(tx.constructor_calldata.into()),
                    nonce_data_availability_mode: match tx.nonce_data_availability_mode {
                        starknet_core::types::DataAvailabilityMode::L1 => {
                            starknet_api::data_availability::DataAvailabilityMode::L1
                        }
                        starknet_core::types::DataAvailabilityMode::L2 => {
                            starknet_api::data_availability::DataAvailabilityMode::L2
                        }
                    },
                    fee_data_availability_mode: match tx.fee_data_availability_mode {
                        starknet_core::types::DataAvailabilityMode::L1 => {
                            starknet_api::data_availability::DataAvailabilityMode::L1
                        }
                        starknet_core::types::DataAvailabilityMode::L2 => {
                            starknet_api::data_availability::DataAvailabilityMode::L2
                        }
                    },
                    paymaster_data: PaymasterData(tx.paymaster_data),
                };
                Ok(StarknetApiTransaction::DeployAccount(
                    DeployAccountTransaction::V3(deploy_account_tx),
                ))
            }
        },
    }
}
