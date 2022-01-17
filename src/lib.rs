use near_contract_standards::non_fungible_token::metadata::NFTContractMetadata;
use near_contract_standards::non_fungible_token::hash_account_id;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault, Promise, CryptoHash, Balance, Gas, PromiseResult};
use near_sdk::collections::{UnorderedSet, UnorderedMap};
use near_sdk::ext_contract;
use serde::{Serialize, Deserialize};

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Marketplaces,
    MarketplacesInner { account_id_hash: CryptoHash },
}

type MarketplaceId = AccountId;

const INITIAL_BALANCE : Balance = 5_000_000_000_000_000_000_000_000;
const NEW_MARKET_GAS : Gas = Gas(100_000_000_000_000);
const ADD_NEW_MARKET_GAS : Gas = Gas(100_000_000_000_000);
const NO_BALANCE: Balance = 0;
const CODE: &[u8] = include_bytes!("./compiled/main.wasm");
const NEW_METHOD : &str = "new";

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    marketplaces: UnorderedMap<AccountId, UnorderedSet<MarketplaceId>>,
}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct NewArgs {
    owner_id: AccountId,
    marketplace_metadata: NFTContractMetadata,
}

#[ext_contract(ext_self)]
trait ExtSelf {
    fn resolve_market_creation(
        &mut self,
        creator_id: AccountId,
        subaccount_id: AccountId
    );
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new() -> Self {
        assert!(!env::state_exists(), "Already initialized");
        Self {
            marketplaces: UnorderedMap::new(StorageKey::Marketplaces.try_to_vec().unwrap())
        }
    }

    #[payable]
    pub fn create_market(&mut self, prefix: AccountId, contract_metadata: NFTContractMetadata) -> Promise {
        let creator_id = env::signer_account_id();
        let subaccount_id: MarketplaceId = format!("{}.{}", prefix, env::current_account_id())
            .parse()
            .unwrap();

        let creators_collections = self
            .marketplaces
            .get(&creator_id.clone())
            .unwrap_or_else(|| {
                UnorderedSet::new(StorageKey::MarketplacesInner {
                    account_id_hash: hash_account_id(&creator_id.clone())
                }.try_to_vec().unwrap()
                )
            });
        env::log_str(&*format!("Spec: {}", contract_metadata.spec));
        assert!(!creators_collections.contains(&subaccount_id.clone()));
        assert_eq!(env::attached_deposit(), INITIAL_BALANCE);

        let args = NewArgs {
            owner_id: creator_id.clone(),
            marketplace_metadata: contract_metadata,
        };
        let serialized_args = near_sdk::borsh::BorshSerialize::try_to_vec(&args)
            .expect("Failed to serialize the cross contract args using Borsh.");

        Promise::new(subaccount_id.clone())
            .create_account()
            .transfer(INITIAL_BALANCE)
            .deploy_contract(CODE.to_vec())
            .then(Promise::new(subaccount_id.clone())
                .function_call(
                    NEW_METHOD.to_string(),
                    serialized_args,
                    NO_BALANCE,
                    NEW_MARKET_GAS
                ))
            .then(ext_self::resolve_market_creation(
                creator_id,
                subaccount_id,
                env::current_account_id(),
                NO_BALANCE,
                ADD_NEW_MARKET_GAS,
            ))
    }

    #[private]
    pub fn resolve_market_creation(
        &mut self,
        creator_id: AccountId,
        subaccount_id: AccountId
    ) {
        assert_eq!(env::promise_results_count(), 1);
        match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Failed => env::panic_str("Creation of collection has failed."),
            PromiseResult::Successful(_) => {
                let mut creators_collections = self
                    .marketplaces
                    .get(&creator_id.clone())
                    .unwrap_or_else(|| {
                        UnorderedSet::new(StorageKey::MarketplacesInner {
                            account_id_hash: hash_account_id(&creator_id.clone())
                        }.try_to_vec().unwrap()
                        )
                    });
                assert!(!creators_collections.contains(&subaccount_id.clone()));
                creators_collections.insert(&subaccount_id.clone());
                self.marketplaces.insert(&creator_id.clone(), &creators_collections);
            }
        }
    }
}