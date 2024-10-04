//! This module contains the functions to generate the transaction data from
//! the RPC response.

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
use starknet_core::types::{ResourceBoundsMapping, Transaction as StarknetCoreTransaction};

use crate::error::RpcClientError;

/// This function converts [`starknet_core::types::ResourceBoundsMapping`] into
/// the equivalent type in crate [`starknet_api`].
///
/// # Arguments
///
/// - `resource_bounds_mapping`: The input object.
fn convert_resource_bounds(
    resource_bounds_mapping: &ResourceBoundsMapping,
) -> Vec<(Resource, ResourceBounds)> {
    let mut resource_bounds = Vec::new();
    let l1_resource = (
        Resource::L1Gas,
        ResourceBounds {
            max_amount: resource_bounds_mapping.l1_gas.max_amount,
            max_price_per_unit: resource_bounds_mapping.l1_gas.max_price_per_unit,
        },
    );
    let l2_resource = (
        Resource::L2Gas,
        ResourceBounds {
            max_amount: resource_bounds_mapping.l2_gas.max_amount,
            max_price_per_unit: resource_bounds_mapping.l2_gas.max_price_per_unit,
        },
    );
    resource_bounds.push(l1_resource);
    resource_bounds.push(l2_resource);
    resource_bounds
}

/// This function converts [`starknet_core::types::DataAvailabilityMode`] into
/// the equivalent type in crate [`starknet_api`].
///
/// # Arguments
///
/// - `data_availability_mode`: The input object.
fn convert_data_availability_mode(
    data_availability_mode: starknet_core::types::DataAvailabilityMode,
) -> starknet_api::data_availability::DataAvailabilityMode {
    match data_availability_mode {
        starknet_core::types::DataAvailabilityMode::L1 => {
            starknet_api::data_availability::DataAvailabilityMode::L1
        }
        starknet_core::types::DataAvailabilityMode::L2 => {
            starknet_api::data_availability::DataAvailabilityMode::L2
        }
    }
}

/// This function converts [`starknet_core::types::InvokeTransaction`] into
/// [`starknet_api::transaction::Transaction`].
///
/// # Arguments
///
/// - `tx`: The input transaction.
///
/// # Errors
///
/// Returns [`Err`] if `transaction` contains invalid numbers that can't be
/// translated to a [`starknet_api::transaction::Transaction`] object.
fn convert_invoke_transaction(
    tx: starknet_core::types::InvokeTransaction,
) -> Result<StarknetApiTransaction, RpcClientError> {
    match tx {
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
            let invoke_tx = InvokeTransactionV3 {
                resource_bounds: convert_resource_bounds(&tx.resource_bounds).try_into()?,
                tip: Tip(tx.tip),
                signature: TransactionSignature(tx.signature),
                nonce: Nonce(tx.nonce),
                sender_address: ContractAddress(tx.sender_address.try_into()?),
                calldata: Calldata(tx.calldata.into()),
                nonce_data_availability_mode: convert_data_availability_mode(
                    tx.nonce_data_availability_mode,
                ),
                fee_data_availability_mode: convert_data_availability_mode(
                    tx.fee_data_availability_mode,
                ),
                paymaster_data: PaymasterData(tx.paymaster_data),
                account_deployment_data: AccountDeploymentData(tx.account_deployment_data),
            };
            Ok(StarknetApiTransaction::Invoke(InvokeTransaction::V3(
                invoke_tx,
            )))
        }
    }
}

/// This function converts [`starknet_core::types::L1HandlerTransaction`] into
/// [`starknet_api::transaction::Transaction`].
///
/// # Arguments
///
/// - `tx`: The input transaction.
///
/// # Errors
///
/// Returns [`Err`] if `transaction` contains invalid numbers that can't be
/// translated to a [`starknet_api::transaction::Transaction`] object.
fn convert_l1_handler_transaction(
    tx: starknet_core::types::L1HandlerTransaction,
) -> Result<StarknetApiTransaction, RpcClientError> {
    let l1handler_tx = L1HandlerTransaction {
        version: TransactionVersion(tx.version),
        nonce: Nonce(tx.nonce.into()),
        contract_address: ContractAddress(tx.contract_address.try_into()?),
        entry_point_selector: EntryPointSelector(tx.entry_point_selector),
        calldata: Calldata(tx.calldata.into()),
    };
    Ok(StarknetApiTransaction::L1Handler(l1handler_tx))
}

/// This function converts [`starknet_core::types::DeclareTransaction`] into
/// [`starknet_api::transaction::Transaction`].
///
/// # Arguments
///
/// - `tx`: The input transaction.
///
/// # Errors
///
/// Returns [`Err`] if `transaction` contains invalid numbers that can't be
/// translated to a [`starknet_api::transaction::Transaction`] object.
fn convert_declare_transaction(
    tx: starknet_core::types::DeclareTransaction,
) -> Result<StarknetApiTransaction, RpcClientError> {
    match tx {
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
            let declare_tx = DeclareTransactionV3 {
                resource_bounds: convert_resource_bounds(&tx.resource_bounds).try_into()?,
                tip: Tip(tx.tip),
                signature: TransactionSignature(tx.signature),
                nonce: Nonce(tx.nonce),
                class_hash: ClassHash(tx.class_hash),
                compiled_class_hash: CompiledClassHash(tx.compiled_class_hash),
                sender_address: ContractAddress(tx.sender_address.try_into()?),
                nonce_data_availability_mode: convert_data_availability_mode(
                    tx.nonce_data_availability_mode,
                ),
                fee_data_availability_mode: convert_data_availability_mode(
                    tx.fee_data_availability_mode,
                ),
                paymaster_data: PaymasterData(tx.paymaster_data),
                account_deployment_data: AccountDeploymentData(tx.account_deployment_data),
            };
            Ok(StarknetApiTransaction::Declare(DeclareTransaction::V3(
                declare_tx,
            )))
        }
    }
}

/// This function converts [`starknet_core::types::DeployTransaction`] into
/// [`starknet_api::transaction::Transaction`].
///
/// # Arguments
///
/// - `tx`: The input transaction.
fn convert_deploy_transaction(
    tx: starknet_core::types::DeployTransaction,
) -> StarknetApiTransaction {
    let deploy_tx = DeployTransaction {
        version: TransactionVersion(tx.version),
        class_hash: ClassHash(tx.class_hash),
        contract_address_salt: ContractAddressSalt(tx.contract_address_salt),
        constructor_calldata: Calldata(tx.constructor_calldata.into()),
    };
    StarknetApiTransaction::Deploy(deploy_tx)
}

/// This function converts [`starknet_core::types::DeployAccountTransaction`]
/// into [`starknet_api::transaction::Transaction`].
///
/// # Arguments
///
/// - `tx`: The input transaction.
///
/// # Errors
///
/// Returns [`Err`] if `transaction` contains invalid numbers that can't be
/// translated to a [`starknet_api::transaction::Transaction`] object.
fn convert_deploy_account_transaction(
    tx: starknet_core::types::DeployAccountTransaction,
) -> Result<StarknetApiTransaction, RpcClientError> {
    match tx {
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
            let deploy_account_tx = DeployAccountTransactionV3 {
                resource_bounds: convert_resource_bounds(&tx.resource_bounds).try_into()?,
                tip: Tip(tx.tip),
                signature: TransactionSignature(tx.signature),
                nonce: Nonce(tx.nonce),
                class_hash: ClassHash(tx.class_hash),
                contract_address_salt: ContractAddressSalt(tx.contract_address_salt),
                constructor_calldata: Calldata(tx.constructor_calldata.into()),
                nonce_data_availability_mode: convert_data_availability_mode(
                    tx.nonce_data_availability_mode,
                ),
                fee_data_availability_mode: convert_data_availability_mode(
                    tx.fee_data_availability_mode,
                ),
                paymaster_data: PaymasterData(tx.paymaster_data),
            };
            Ok(StarknetApiTransaction::DeployAccount(
                DeployAccountTransaction::V3(deploy_account_tx),
            ))
        }
    }
}

/// This function converts [`starknet_core::types::Transaction`] into
/// [`starknet_api::transaction::Transaction`].
///
/// # Arguments
///
/// - `tx`: The transaction object.
///
/// # Errors
///
/// Returns [`Err`] if `transaction` contains invalid numbers.
pub fn convert_transaction(
    tx: StarknetCoreTransaction,
) -> Result<StarknetApiTransaction, RpcClientError> {
    match tx {
        StarknetCoreTransaction::Invoke(tx) => convert_invoke_transaction(tx),
        StarknetCoreTransaction::L1Handler(tx) => convert_l1_handler_transaction(tx),
        StarknetCoreTransaction::Declare(tx) => convert_declare_transaction(tx),
        StarknetCoreTransaction::Deploy(tx) => Ok(convert_deploy_transaction(tx)),
        StarknetCoreTransaction::DeployAccount(tx) => convert_deploy_account_transaction(tx),
    }
}
