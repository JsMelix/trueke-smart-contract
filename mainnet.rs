#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Symbol,
};

#[contracttype]
#[derive(Clone)]
pub struct Escrow {
    pub id: u32,
    pub party_a: Address,
    pub party_b: Address,
    pub amount: i128,           // Diferencia a pagar (puede ser 0)
    pub token: Address,         // XLM, USDC, $TRUEKE, etc.
    pub confirmed_a: bool,
    pub confirmed_b: bool,
    pub deadline: u64,
    pub is_active: bool,
}

#[contracttype]
pub enum DataKey {
    Escrow(u32),
    NextEscrowId,
    Admin,
}

#[contract]
pub struct TruekeEscrow;

#[contractimpl]
impl TruekeEscrow {

    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn create_escrow(
        env: Env,
        party_a: Address,
        party_b: Address,
        amount: i128,
        token: Address,
        deadline_days: u32,
    ) -> u32 {
        party_a.require_auth();
        party_b.require_auth();

        let mut next_id: u32 = env.storage().instance().get(&DataKey::NextEscrowId).unwrap_or(1);
        
        let deadline = env.ledger().timestamp() + (deadline_days as u64 * 86400);

        let escrow = Escrow {
            id: next_id,
            party_a: party_a.clone(),
            party_b: party_b.clone(),
            amount,
            token: token.clone(),
            confirmed_a: false,
            confirmed_b: false,
            deadline,
            is_active: true,
        };

        env.storage().instance().set(&DataKey::Escrow(next_id), &escrow);
        env.storage().instance().set(&DataKey::NextEscrowId, &(next_id + 1));

        env.events().publish(
            (Symbol::new(&env, "escrow_created"),),
            (next_id, &party_a, &party_b, amount),
        );

        next_id
    }

    pub fn deposit(env: Env, escrow_id: u32, from: Address) {
        from.require_auth();

        let mut escrow: Escrow = env.storage().instance().get(&DataKey::Escrow(escrow_id))
            .expect("Escrow no existe");

        if !escrow.is_active { panic!("Escrow finalizado"); }
        if from != escrow.party_b { panic!("Solo Party B puede depositar"); }

        let token_client = token::TokenClient::new(&env, &escrow.token);
        token_client.transfer(&from, &env.current_contract_address(), &escrow.amount);
    }

    pub fn confirm_receipt(env: Env, escrow_id: u32, caller: Address) {
        caller.require_auth();

        let mut escrow: Escrow = env.storage().instance().get(&DataKey::Escrow(escrow_id))
            .expect("Escrow no existe");

        if caller == escrow.party_a {
            escrow.confirmed_a = true;
        } else if caller == escrow.party_b {
            escrow.confirmed_b = true;
        } else {
            panic!("No autorizado");
        }

        env.storage().instance().set(&DataKey::Escrow(escrow_id), &escrow);

        if escrow.confirmed_a && escrow.confirmed_b {
            Self::release(&env, escrow_id, escrow);
        }
    }

    fn release(env: &Env, escrow_id: u32, mut escrow: Escrow) {
        let token_client = token::TokenClient::new(env, &escrow.token);
        
        if escrow.amount > 0 {
            token_client.transfer(
                &env.current_contract_address(),
                &escrow.party_a,
                &escrow.amount,
            );
        }

        escrow.is_active = false;
        env.storage().instance().set(&DataKey::Escrow(escrow_id), &escrow);

        env.events().publish(
            (Symbol::new(env, "escrow_completed"),),
            (escrow_id, &escrow.party_a, &escrow.party_b),
        );
    }

    pub fn refund(env: Env, escrow_id: u32) {
        let escrow: Escrow = env.storage().instance().get(&DataKey::Escrow(escrow_id))
            .expect("Escrow no existe");

        if env.ledger().timestamp() > escrow.deadline && escrow.is_active {
            let token_client = token::TokenClient::new(&env, &escrow.token);
            
            if escrow.amount > 0 {
                token_client.transfer(
                    &env.current_contract_address(),
                    &escrow.party_b,
                    &escrow.amount,
                );
            }

            let mut updated = escrow;
            updated.is_active = false;
            env.storage().instance().set(&DataKey::Escrow(escrow_id), &updated);
        }
    }
}
