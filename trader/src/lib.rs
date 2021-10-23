//! A proof of concept of the Autonomy network on NEAR
//!
//! 0. Allows a user to deposit NEAR/nDAI into a contract, and specify a range at which they would like to buy/sell Near,
//! 1. relies on croncat to ping to update contracts every 10 minutes to
//! 2. reference the NEAR price, via chainlink oracle, against that range and
//! 3. trade NEAR against nDAI if near crosses the reference, while
//! 4. collecting 0.1 NEAR from the user for each trade executed.

#![allow(unused_imports)]
use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, UnorderedMap};
use near_sdk::env::STORAGE_PRICE_PER_BYTE;
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{env, log, near_bindgen, AccountId, Balance, PanicOnDefault, PromiseOrValue};
pub const OPEN_POSITION_STORAGE_COST: Balance = 600 * STORAGE_PRICE_PER_BYTE; // estimate 600 bytes per user

near_sdk::setup_alloc!();

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    owner_id: AccountId,
    // note: simplicity over optimization
    accounts: UnorderedMap<AccountId, User>,
    // todo: add some auto-specific fungible-token functionality
}

/// A user's balance in Near and nDAI
#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct User {
    near_balance: Balance,
    ndai_balance: Balance,
    range: Range,
}
impl User {
    // todo: add functionality allowing user to deposit nDAI when opening position
    fn new(near_deposit: Balance, buy_point: Balance, sell_point: Balance) -> Self {
        assert!(buy_point > sell_point);
        Self {
            near_balance: near_deposit,
            ndai_balance: 0,
            range: Range::new(buy_point, sell_point),
        }
    }
    fn update_balances(&mut self, near_balance: Balance, ndai_balance: Balance) {
        self.near_balance = near_balance;
        self.ndai_balance = ndai_balance;
    }
}

/// User specified buy/sell points for the NEAR-nDAI pair
#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct Range {
    /// Buy NEAR if price dips below:
    buy_point: Balance,
    /// Sell NEAR if price goes above:
    sell_point: Balance,
}
impl Range {
    fn new(buy_point: Balance, sell_point: Balance) -> Self {
        assert!(buy_point > sell_point);
        Self {
            buy_point,
            sell_point,
        }
    }
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract with the given `owner_id`
    #[init]
    pub fn new(owner_id: ValidAccountId) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        Self {
            owner_id: owner_id.into(),
            accounts: UnorderedMap::new(b"a"),
        }
    }

    #[payable]
    pub fn open_position(&mut self, buy_point: Balance, sell_point: Balance) {
        let predecessor = env::predecessor_account_id();
        assert!(self.accounts.get(&predecessor).is_none());
        // validate storage is covered
        assert!(env::attached_deposit() > OPEN_POSITION_STORAGE_COST);
        let deposit = env::attached_deposit() - OPEN_POSITION_STORAGE_COST;
        self.accounts
            .insert(&predecessor, &User::new(deposit, buy_point, sell_point));
    }

    pub fn change_position(&mut self, buy_point: Balance, sell_point: Balance) {
        let predecessor = env::predecessor_account_id();
        let mut user = self.accounts.get(&predecessor).expect("no such user");
        user.range.buy_point = buy_point;
        user.range.sell_point = sell_point;
        self.accounts.insert(&predecessor, &user);
    }

    /// Remove account, send user their funds
    pub fn close_position(&mut self) {
        let predecessor = env::predecessor_account_id();
        let user = self.accounts.get(&predecessor).expect("no such user");

        self.accounts.remove(&predecessor);
        near_sdk::Promise::new(predecessor)
            .transfer(user.near_balance + OPEN_POSITION_STORAGE_COST);
        // todo: cross-contract update ndai balance
    }

    /// update all user positions. Croncat entrypoint. Example, to call every 5m, to run , call:
    /// near call cron.in.testnet create_task '{"contract_id": "<THIS_CONTRACT_ADDRESS>","function_id": "ping","cadence": "* */5 * * * *","recurring": true,"deposit": 0,"gas": 2400000000000}' --accountId <YOUR_ACCOUNT>.testnet --amount 2
    pub fn ping(&mut self) {
        assert_eq!(env::predecessor_account_id(), "cron.in.testnet"); // croncat testnet addres
        let price = get_near_ndai_price();
        let mut updates = std::collections::HashMap::new();

        for (account, mut user) in self.accounts.iter() {
            if user.range.buy_point > price && user.ndai_balance > 0 {
                // buy
                let updated_near_balance = user.ndai_balance * price;
                user.update_balances(updated_near_balance, 0);
                updates.insert(account, user);
            } else if user.range.sell_point < price && user.near_balance > 0 {
                // sell
                let updated_ndai_balance = user.near_balance / price;
                user.update_balances(0, updated_ndai_balance);
                updates.insert(account, user);
            }
        }
        for (account, user) in updates {
            self.accounts.insert(&account, &user);
        }
    }
}

// todo: get NEAR:nDAI price from chainlink oracle
fn get_near_ndai_price() -> Balance {
    todo!();
}
