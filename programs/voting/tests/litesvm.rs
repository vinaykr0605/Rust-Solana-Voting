use borsh::{BorshDeserialize, BorshSerialize};
use litesvm::LiteSVM;
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};

const PROGRAM_ID: Pubkey = solana_sdk::pubkey!("ChvcxPhLTMi8QuzEBjdxaLVDeaEczUAKhtosBGvpKwmk");

#[derive(BorshDeserialize, BorshSerialize, Debug)]
struct PollAccount {
    pub poll_name: String,
    pub poll_description: String,
    pub poll_voting_start: u64,
    pub poll_voting_end: u64,
    pub poll_option_index: u64,
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
struct CandidateAccount {
    pub candidate_name: String,
    pub candidate_description: String,
    pub candidate_votes: u64,
}

fn get_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", name));
    let result = hasher.finalize();
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&result[..8]);
    discriminator
}

fn setup_svm() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    // Load program
    let program_bytes = include_bytes!("../../../target/deploy/voting.so");
    svm.add_program(PROGRAM_ID, program_bytes);

    (svm, payer)
}

#[test]
fn test_init_poll() {
    let (mut svm, payer) = setup_svm();

    let poll_id = 1u64;
    let (poll_pda, _bump) = Pubkey::find_program_address(
        &[b"poll", &poll_id.to_le_bytes()],
        &PROGRAM_ID,
    );

    let name = "Test Poll".to_string();
    let description = "Description".to_string();
    let start = 0u64;
    let end = 1000u64;

    let discriminator = get_discriminator("init_poll");
    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&poll_id.to_le_bytes());
    data.extend_from_slice(&start.to_le_bytes());
    data.extend_from_slice(&end.to_le_bytes());
    
    // Serialize strings (len: u32 + bytes)
    data.extend_from_slice(&(name.len() as u32).to_le_bytes());
    data.extend_from_slice(name.as_bytes());
    data.extend_from_slice(&(description.len() as u32).to_le_bytes());
    data.extend_from_slice(description.as_bytes());

    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(poll_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).expect("Transaction failed");

    let account = svm.get_account(&poll_pda).expect("Account not found");
    let poll_data = PollAccount::deserialize(&mut &account.data[8..]).expect("Deserialization failed");

    println!("Poll Initialized:");
    println!("  Address: {}", poll_pda);
    println!("  Name: {}", poll_data.poll_name);
    println!("  Description: {}", poll_data.poll_description);
    println!("  Window: {} -> {}", poll_data.poll_voting_start, poll_data.poll_voting_end);

    assert_eq!(poll_data.poll_name, name);
    assert_eq!(poll_data.poll_description, description);
    assert_eq!(poll_data.poll_voting_start, start);
    assert_eq!(poll_data.poll_voting_end, end);
}

#[test]
fn test_initialize_candidate() {
    let (mut svm, payer) = setup_svm();

    let poll_id = 1u64;
    let (poll_pda, _) = Pubkey::find_program_address(&[b"poll", &poll_id.to_le_bytes()], &PROGRAM_ID);

    // 1. Init Poll
    let init_poll_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(poll_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: {
            let mut d = Vec::new();
            d.extend_from_slice(&get_discriminator("init_poll"));
            d.extend_from_slice(&poll_id.to_le_bytes());
            d.extend_from_slice(&0u64.to_le_bytes()); // start
            d.extend_from_slice(&1000u64.to_le_bytes()); // end
            d.extend_from_slice(&9u32.to_le_bytes()); d.extend_from_slice(b"Test Poll");
            d.extend_from_slice(&11u32.to_le_bytes()); d.extend_from_slice(b"Description");
            d
        },
    };
    svm.send_transaction(Transaction::new_signed_with_payer(&[init_poll_ix], Some(&payer.pubkey()), &[&payer], svm.latest_blockhash())).unwrap();

    // 2. Initialize Candidate
    let candidate_name = "Alice".to_string();
    let candidate_desc = "First Candidate".to_string();
    let (candidate_pda, _) = Pubkey::find_program_address(
        &[&poll_id.to_le_bytes(), candidate_name.as_bytes()],
        &PROGRAM_ID,
    );

    println!("Initializing Candidate: {}...", candidate_name);

    let mut data = Vec::new();
    data.extend_from_slice(&get_discriminator("initialize_candidate"));
    data.extend_from_slice(&(candidate_name.len() as u32).to_le_bytes());
    data.extend_from_slice(candidate_name.as_bytes());
    data.extend_from_slice(&poll_id.to_le_bytes());
    data.extend_from_slice(&(candidate_desc.len() as u32).to_le_bytes());
    data.extend_from_slice(candidate_desc.as_bytes());

    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(poll_pda, false),
            AccountMeta::new(candidate_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };

    svm.send_transaction(Transaction::new_signed_with_payer(&[instruction], Some(&payer.pubkey()), &[&payer], svm.latest_blockhash())).expect("Candidate init failed");

    let account = svm.get_account(&candidate_pda).expect("Candidate account not found");
    let candidate_data = CandidateAccount::deserialize(&mut &account.data[8..]).expect("Deserialization failed");

    println!("Candidate Initialized:");
    println!("  Name: {}", candidate_data.candidate_name);
    println!("  Votes: {}", candidate_data.candidate_votes);

    assert_eq!(candidate_data.candidate_name, candidate_name);
    assert_eq!(candidate_data.candidate_votes, 0);
}

#[test]
fn test_vote() {
    let (mut svm, payer) = setup_svm();

    let poll_id = 1u64;
    let (poll_pda, _) = Pubkey::find_program_address(&[b"poll", &poll_id.to_le_bytes()], &PROGRAM_ID);

    // 1. Init Poll (start: 0, end: 1000)
    let init_poll_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(poll_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: {
            let mut d = Vec::new();
            d.extend_from_slice(&get_discriminator("init_poll"));
            d.extend_from_slice(&poll_id.to_le_bytes());
            d.extend_from_slice(&0u64.to_le_bytes()); // start
            d.extend_from_slice(&1000u64.to_le_bytes()); // end
            d.extend_from_slice(&9u32.to_le_bytes()); d.extend_from_slice(b"Test Poll");
            d.extend_from_slice(&11u32.to_le_bytes()); d.extend_from_slice(b"Description");
            d
        },
    };
    svm.send_transaction(Transaction::new_signed_with_payer(&[init_poll_ix], Some(&payer.pubkey()), &[&payer], svm.latest_blockhash())).unwrap();

    // 2. Init Candidate
    let candidate_name = "Alice".to_string();
    let (candidate_pda, _) = Pubkey::find_program_address(&[&poll_id.to_le_bytes(), candidate_name.as_bytes()], &PROGRAM_ID);
    let init_cand_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(poll_pda, false),
            AccountMeta::new(candidate_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: {
            let mut d = Vec::new();
            d.extend_from_slice(&get_discriminator("initialize_candidate"));
            d.extend_from_slice(&(candidate_name.len() as u32).to_le_bytes());
            d.extend_from_slice(candidate_name.as_bytes());
            d.extend_from_slice(&poll_id.to_le_bytes());
            d.extend_from_slice(&1u32.to_le_bytes()); d.extend_from_slice(b"X");
            d
        },
    };
    svm.send_transaction(Transaction::new_signed_with_payer(&[init_cand_ix], Some(&payer.pubkey()), &[&payer], svm.latest_blockhash())).unwrap();

    // 3. Vote
    println!("Casting vote for {}...", candidate_name);
    let mut vote_data = Vec::new();
    vote_data.extend_from_slice(&get_discriminator("vote"));
    vote_data.extend_from_slice(&(candidate_name.len() as u32).to_le_bytes());
    vote_data.extend_from_slice(candidate_name.as_bytes());
    vote_data.extend_from_slice(&poll_id.to_le_bytes());

    let vote_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(poll_pda, false),
            AccountMeta::new(candidate_pda, false),
        ],
        data: vote_data,
    };

    svm.send_transaction(Transaction::new_signed_with_payer(&[vote_ix], Some(&payer.pubkey()), &[&payer], svm.latest_blockhash())).expect("Vote failed");

    let account = svm.get_account(&candidate_pda).unwrap();
    let candidate_data = CandidateAccount::deserialize(&mut &account.data[8..]).unwrap();
    
    println!("Vote success! New vote count: {}", candidate_data.candidate_votes);
    
    assert_eq!(candidate_data.candidate_votes, 1);
}
