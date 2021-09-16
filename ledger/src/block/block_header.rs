// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

use crate::{
    posw::{txids_to_roots, PoswMarlin},
    BlockHeaderHash,
    BlockHeaderMetadata,
    MerkleRoot,
    Network,
    PedersenMerkleRoot,
    ProofOfSuccinctWork,
    Transactions,
};
use snarkvm_algorithms::{merkle_tree::MerkleTree, CRH};
use snarkvm_dpc::{Parameters, TransactionScheme};
use snarkvm_utilities::{FromBytes, ToBytes};

use anyhow::{anyhow, Result};
use rand::{CryptoRng, Rng};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Result as IoResult, Write},
    sync::Arc,
};

/// Block header.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlockHeader<N: Network> {
    /// Hash of the previous block - 32 bytes
    pub previous_block_hash: BlockHeaderHash,
    /// Merkle root representing the transactions in the block - 32 bytes
    pub transactions_root: PedersenMerkleRoot,
    /// The Merkle root representing the ledger commitments - 32 bytes
    pub commitments_root: MerkleRoot,
    /// The Merkle root representing the ledger serial numbers - 32 bytes
    pub serial_numbers_root: MerkleRoot,
    /// The block header metadata - 20 bytes
    pub metadata: BlockHeaderMetadata,
    /// Proof of Succinct Work
    pub proof: ProofOfSuccinctWork<N>,
}

impl<N: Network> BlockHeader<N> {
    /// Initializes a new instance of a block header.
    pub fn new<T: TransactionScheme, R: Rng + CryptoRng>(
        previous_block_hash: BlockHeaderHash,
        transactions: &Transactions<T>,
        commitments_root: MerkleRoot,
        serial_numbers_root: MerkleRoot,
        timestamp: i64,
        difficulty_target: u64,
        max_nonce: u32,
        rng: &mut R,
    ) -> Result<Self> {
        assert!(!(*transactions).is_empty(), "Cannot create block with no transactions");

        let txids = transactions.to_transaction_ids()?;
        let (_, transactions_root, subroots) = txids_to_roots(&txids);

        // TODO (howardwu): TEMPORARY - Make this a static once_cell.
        // Mine the block.
        let posw = PoswMarlin::load()?;
        let (nonce, proof) = posw.mine(&subroots, difficulty_target, rng, max_nonce)?;

        Ok(Self {
            previous_block_hash,
            transactions_root,
            commitments_root,
            serial_numbers_root,
            metadata: BlockHeaderMetadata::new(timestamp, difficulty_target, nonce),
            proof: FromBytes::read_le(&proof[..])?,
        })
    }

    /// Initializes a new instance of a genesis block header.
    pub fn new_genesis<T: TransactionScheme, C: Parameters, R: Rng + CryptoRng>(
        transactions: &Transactions<T>,
        rng: &mut R,
    ) -> Result<Self> {
        let previous_block_hash = BlockHeaderHash([0u8; 32]);

        // Craft the commitments root from the transactions
        let transaction_commitments: Vec<&<T as TransactionScheme>::Commitment> =
            transactions.0.iter().map(|t| t.commitments()).flatten().collect();
        let record_commitment_tree = MerkleTree::new(
            Arc::new(C::record_commitment_tree_parameters().clone()),
            &transaction_commitments,
        )?;
        let commitments_root = MerkleRoot::from_element(record_commitment_tree.root());

        // Craft the serial numbers root from the transactions
        let transaction_serial_numbers: Vec<&<T as TransactionScheme>::SerialNumber> =
            transactions.0.iter().map(|t| t.serial_numbers()).flatten().collect();
        let record_serial_numbers_tree = MerkleTree::new(
            Arc::new(C::record_commitment_tree_parameters().clone()),
            &transaction_serial_numbers,
        )?;
        let serial_numbers_root = MerkleRoot::from_element(record_serial_numbers_tree.root());

        let timestamp = 0i64;
        let difficulty_target = u64::MAX;
        let max_nonce = u32::MAX;

        let block_header = Self::new(
            previous_block_hash,
            transactions,
            commitments_root,
            serial_numbers_root,
            timestamp,
            difficulty_target,
            max_nonce,
            rng,
        )?;

        match block_header.is_genesis() {
            true => Ok(block_header),
            false => Err(anyhow!("Failed to initialize a genesis block header")),
        }
    }

    /// Returns `true` if the block header is a genesis block header.
    pub fn is_genesis(&self) -> bool {
        // Ensure the timestamp in the genesis block is 0.
        self.metadata.timestamp() == 0
            // Ensure the previous block hash in the genesis block is 0.
            || self.previous_block_hash == BlockHeaderHash([0u8; 32])
    }

    pub fn to_hash(&self) -> Result<BlockHeaderHash> {
        let serialized = self.to_bytes_le()?;
        let hash_bytes = N::block_header_crh().hash(&serialized)?.to_bytes_le()?;

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_bytes);

        Ok(BlockHeaderHash(hash))
    }

    /// Returns the block header size in bytes - 919 bytes.
    pub fn size() -> usize {
        BlockHeaderHash::size()
            + PedersenMerkleRoot::size()
            + MerkleRoot::size()
            + MerkleRoot::size()
            + BlockHeaderMetadata::size()
            + ProofOfSuccinctWork::<N>::size()
    }
}

impl<N: Network> FromBytes for BlockHeader<N> {
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let previous_block_hash = <[u8; 32]>::read_le(&mut reader)?;
        let transactions_root = <[u8; 32]>::read_le(&mut reader)?;
        let commitments_root = <[u8; 32]>::read_le(&mut reader)?;
        let serial_numbers_root = <[u8; 32]>::read_le(&mut reader)?;
        let metadata = BlockHeaderMetadata::read_le(&mut reader)?;
        let proof = ProofOfSuccinctWork::read_le(&mut reader)?;

        Ok(Self {
            previous_block_hash: BlockHeaderHash(previous_block_hash),
            transactions_root: PedersenMerkleRoot(transactions_root),
            commitments_root: MerkleRoot(commitments_root),
            serial_numbers_root: MerkleRoot(serial_numbers_root),
            metadata,
            proof,
        })
    }
}

impl<N: Network> ToBytes for BlockHeader<N> {
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.previous_block_hash.0.write_le(&mut writer)?;
        self.transactions_root.0.write_le(&mut writer)?;
        self.commitments_root.0.write_le(&mut writer)?;
        self.serial_numbers_root.0.write_le(&mut writer)?;
        self.metadata.write_le(&mut writer)?;
        self.proof.write_le(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testnet2::Testnet2;
    use snarkvm_dpc::{testnet2::Testnet2Parameters, Transaction};
    use snarkvm_parameters::{testnet2::Transaction1, Genesis};

    use chrono::Utc;
    use rand::thread_rng;

    #[test]
    fn test_block_header_genesis() {
        let block_header = BlockHeader::<Testnet2>::new_genesis::<_, Testnet2Parameters, _>(
            &Transactions::from(&[
                Transaction::<Testnet2Parameters>::from_bytes_le(&Transaction1::load_bytes()).unwrap(),
            ]),
            &mut thread_rng(),
        )
        .unwrap();
        assert!(block_header.is_genesis());

        // Ensure the genesis block contains the following.
        assert_eq!(block_header.previous_block_hash, BlockHeaderHash([0u8; 32]));
        assert_eq!(block_header.metadata.timestamp(), 0);
        assert_eq!(block_header.metadata.difficulty_target(), u64::MAX);

        // Ensure the genesis block does *not* contain the following.
        assert_ne!(block_header.transactions_root, PedersenMerkleRoot([0u8; 32]));
        assert_ne!(block_header.commitments_root, MerkleRoot([0u8; 32]));
        assert_ne!(block_header.serial_numbers_root, MerkleRoot([0u8; 32]));
        assert_ne!(
            block_header.proof,
            ProofOfSuccinctWork::new(&vec![0u8; ProofOfSuccinctWork::<Testnet2>::size()]),
        );
    }

    #[test]
    fn test_block_header_serialization() {
        let block_header = BlockHeader::<Testnet2> {
            previous_block_hash: BlockHeaderHash([0u8; 32]),
            transactions_root: PedersenMerkleRoot([0u8; 32]),
            commitments_root: MerkleRoot([0u8; 32]),
            serial_numbers_root: MerkleRoot([0u8; 32]),
            metadata: BlockHeaderMetadata::new(Utc::now().timestamp(), 0u64, 0u32),
            proof: ProofOfSuccinctWork::new(&vec![0u8; ProofOfSuccinctWork::<Testnet2>::size()]),
        };

        let serialized = block_header.to_bytes_le().unwrap();
        assert_eq!(&serialized[..], &bincode::serialize(&block_header).unwrap()[..]);

        let deserialized = BlockHeader::read_le(&serialized[..]).unwrap();
        assert_eq!(deserialized, block_header);
    }

    #[test]
    fn test_block_header_size() {
        let block_header = BlockHeader::<Testnet2> {
            previous_block_hash: BlockHeaderHash([0u8; 32]),
            transactions_root: PedersenMerkleRoot([0u8; 32]),
            commitments_root: MerkleRoot([0u8; 32]),
            serial_numbers_root: MerkleRoot([0u8; 32]),
            metadata: BlockHeaderMetadata::new(Utc::now().timestamp(), 0u64, 0u32),
            proof: ProofOfSuccinctWork::new(&vec![0u8; ProofOfSuccinctWork::<Testnet2>::size()]),
        };
        assert_eq!(
            block_header.to_bytes_le().unwrap().len(),
            BlockHeader::<Testnet2>::size()
        );
    }
}
