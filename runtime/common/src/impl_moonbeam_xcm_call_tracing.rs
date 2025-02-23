// Copyright 2019-2022 PureStake Inc.
// This file is part of Moonbeam.

// Moonbeam is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Moonbeam is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Moonbeam.  If not, see <http://www.gnu.org/licenses/>.

#[macro_export]
macro_rules! impl_moonbeam_xcm_call_tracing {
	{} => {

		type CallResult =
			Result<PostDispatchInfoOf<Call>, DispatchErrorWithPostInfo<PostDispatchInfoOf<Call>>>;

		pub struct MoonbeamCall;
		impl CallDispatcher<Call> for MoonbeamCall {
			fn dispatch(
				call: Call,
				origin: Origin,
			) -> CallResult {
				if let Ok(raw_origin) = TryInto::<RawOrigin<AccountId>>::try_into(origin.clone().caller) {
					match (call.clone(), raw_origin) {
						(
							Call::EthereumXcm(pallet_ethereum_xcm::Call::transact { xcm_transaction }) |
							Call::EthereumXcm(pallet_ethereum_xcm::Call::transact_through_proxy {
								xcm_transaction, ..
							 }),
							RawOrigin::Signed(account_id)
						) => {
							use crate::EthereumXcm;
							use moonbeam_evm_tracer::tracer::EvmTracer;
							use xcm_primitives::{
								XcmToEthereum,
								EthereumXcmTracingStatus,
								ETHEREUM_XCM_TRACING_STORAGE_KEY
							};
							use frame_support::storage::unhashed;

							let dispatch_call = || {
								Call::dispatch(
									call,
									pallet_ethereum_xcm::Origin::XcmEthereumTransaction(
										account_id.into()
									).into()
								)
							};

							return match unhashed::get(
								ETHEREUM_XCM_TRACING_STORAGE_KEY
							) {
								// This runtime instance is used for tracing.
								Some(transaction) => match transaction {
									// Tracing a block, all calls are done using environmental.
									EthereumXcmTracingStatus::Block => {
										let mut res: Option<CallResult> = None;
										EvmTracer::new().trace(|| {
											res = Some(dispatch_call());
										});
										res.expect("Invalid dispatch result")
									},
									// Tracing a transaction, the one matching the trace request
									// is done using environmental, the rest dispatched normally.
									EthereumXcmTracingStatus::Transaction(traced_transaction_hash) => {
										let transaction_hash = xcm_transaction.into_transaction_v2(
											EthereumXcm::nonce()
										)
										.expect("Invalid transaction conversion")
										.hash();
										if transaction_hash == traced_transaction_hash {
											let mut res: Option<CallResult> = None;
											EvmTracer::new().trace(|| {
												res = Some(dispatch_call());
											});
											// Tracing runtime work is done, just signal instance exit.
											unhashed::put::<EthereumXcmTracingStatus>(
												xcm_primitives::ETHEREUM_XCM_TRACING_STORAGE_KEY,
												&EthereumXcmTracingStatus::TransactionExited,
											);
											return res.expect("Invalid dispatch result");
										}
										dispatch_call()
									},
									_ => unreachable!()
								},
								// This runtime instance is importing a block.
								None => dispatch_call()
							};
						},
						_ => {}
					}
				}
				Call::dispatch(call, origin)
			}
		}
	}
}
