use soroban_sdk::{contract, contractimpl, Env, Address, Vec, log};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct Loan {
    borrower: Address,
    amount: i64,
    interest_rate: f32,
    repaid_amount: i64,
    savings: i64,
    is_active: bool,
}

#[derive(Serialize, Deserialize)]
struct LendingPool {
    total_funds: i64,
    loans: Vec<Loan>,
    insurance_fund: i64,
    base_interest_rate: f32,
}

#[contract]
pub struct MicroLoanContract {
    pool: LendingPool,
    owner: Address,
}

#[contractimpl]
impl MicroLoanContract {
    pub fn initialize(env: Env, owner: Address, initial_funds: i64, base_rate: f32) -> Self {
        let pool = LendingPool {
            total_funds: initial_funds,
            loans: Vec::new(&env),
            insurance_fund: initial_funds / 10,
            base_interest_rate: base_rate,
        };
        Self { pool, owner }
    }

    pub fn request_loan(&mut self, env: Env, borrower: Address, amount: i64) -> Result<(), &'static str> {
        borrower.require_auth();
        if amount < 10_000_000 || amount > 1_000_000_000 {
            return Err("Loan amount must be between 1 XLM and 100 XLM");
        }
        if self.pool.total_funds < amount {
            return Err("Insufficient funds in pool");
        }
        if !self.check_blend_pool_availability(amount) {
            return Err("Blend pool unavailable");
        }
        let interest_rate = self.calculate_interest_rate();
        let loan = Loan {
            borrower,
            amount,
            interest_rate,
            repaid_amount: 0,
            savings: 0,
            is_active: true,
        };
        self.pool.total_funds -= amount;
        self.pool.loans.push_back(loan);
        log!(&env, "Loan requested: {} stroops by {}", amount, borrower);
        Ok(())
    }

    pub fn repay_loan(&mut self, env: Env, borrower: Address, amount: i64) -> Result<(), &'static str> {
        borrower.require_auth();
        let loan = self.pool.loans.iter_mut().find(|l| l.borrower == borrower && l.is_active);
        match loan {
            Some(loan) => {
                if amount <= 0 {
                    return Err("Invalid repayment amount");
                }
                loan.repaid_amount += amount;
                let savings = amount / 20;
                loan.savings += savings;
                if loan.savings >= 100_000_000 && loan.interest_rate > 0.5 {
                    loan.interest_rate -= 0.5;
                    log!(&env, "Reward: Interest rate reduced to {}", loan.interest_rate);
                }
                self.pool.total_funds += amount - savings;
                self.pool.insurance_fund += savings / 2;
                if loan.repaid_amount >= loan.amount {
                    loan.is_active = false;
                    log!(&env, "Loan fully repaid by {}", borrower);
                }
                Ok(())
            }
            None => Err("No active loan found"),
        }
    }

    fn calculate_interest_rate(&self) -> f32 {
        let utilization = self.pool.loans.iter().filter(|l| l.is_active).map(|l| l.amount).sum::<i64>() as f32 / self.pool.total_funds as f32;
        self.pool.base_interest_rate + (utilization * 2.0).min(5.0)
    }

    fn check_blend_pool_availability(&self, amount: i64) -> bool {
        self.pool.total_funds >= amount
    }

    pub fn get_loan_status(&self, borrower: Address) -> Option<Loan> {
        self.pool.loans.iter().find(|l| l.borrower == borrower && l.is_active).cloned()
    }

    pub fn get_pool_stats(&self) -> (i64, u32, i64) {
        let active_loans = self.pool.loans.iter().filter(|l| l.is_active).count() as u32;
        let total_savings = self.pool.loans.iter().map(|l| l.savings).sum::<i64>();
        (self.pool.total_funds, active_loans, total_savings)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use soroban_sdk::testutils::{Address as _, Ledger};

        #[test]
        fn test_loan_lifecycle() {
            let env = Env::default();
            let owner = Address::random(&env);
            let contract = MicroLoanContract::initialize(&env, owner.clone(), 10_000_000_000, 5.0);
            let borrower = Address::random(&env);
            let mut contract = contract;

            assert!(contract.request_loan(&env, borrower.clone(), 500_000_000).is_ok());
            let loan = contract.get_loan_status(borrower.clone()).unwrap();
            assert_eq!(loan.amount, 500_000_000);
            assert_eq!(loan.is_active, true);

            assert!(contract.repay_loan(&env, borrower.clone(), 100_000_000).is_ok());
            let loan = contract.get_loan_status(borrower.clone()).unwrap();
            assert_eq!(loan.repaid_amount, 100_000_000);
            assert_eq!(loan.savings, 5_000_000);
        }
    }