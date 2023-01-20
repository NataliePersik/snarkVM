// Copyright (C) 2019-2022 Aleo Systems Inc.
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

use super::*;

use snarkvm_utilities::DeserializeExt;

impl<N: Network> Serialize for ProverSolution<N> {
    /// Serializes the prover solution to a JSON-string or buffer.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match serializer.is_human_readable() {
            true => {
                let mut prover_solution = serializer.serialize_struct("ProverSolution", 3)?;
                prover_solution.serialize_field("partial_solution", &self.partial_solution)?;
                prover_solution.serialize_field("proof.w", &self.proof.w)?;
                if let Some(random_v) = &self.proof.random_v {
                    prover_solution.serialize_field("proof.random_v", &random_v)?;
                }
                prover_solution.end()
            }
            false => ToBytesSerializer::serialize_with_size_encoding(self, serializer),
        }
    }
}

impl<'de, N: Network> Deserialize<'de> for ProverSolution<N> {
    /// Deserializes the prover solution from a JSON-string or buffer.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.is_human_readable() {
            true => {
                let mut prover_solution = serde_json::Value::deserialize(deserializer)?;
                Ok(Self::new(
                    DeserializeExt::take_from_value::<D>(&mut prover_solution, "partial_solution")?,
                    KZGProof {
                        w: DeserializeExt::take_from_value::<D>(&mut prover_solution, "proof.w")?,
                        random_v: serde_json::from_value(
                            prover_solution.get_mut("proof.random_v").unwrap_or(&mut serde_json::Value::Null).take(),
                        )
                        .map_err(de::Error::custom)?,
                    },
                ))
            }
            false => FromBytesDeserializer::<Self>::deserialize_with_size_encoding(deserializer, "prover solution"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console::{account::PrivateKey, network::Testnet3};

    type CurrentNetwork = Testnet3;

    #[test]
    fn test_serde_json() -> Result<()> {
        let mut rng = TestRng::default();
        let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
        let address = Address::try_from(private_key)?;

        // Sample a new prover puzzle solution.
        let partial_solution = PartialSolution::new(address, u64::rand(&mut rng), KZGCommitment(rng.gen()));
        let expected = ProverSolution::new(partial_solution, KZGProof { w: rng.gen(), random_v: None });

        // Serialize
        let expected_string = &expected.to_string();
        let candidate_string = serde_json::to_string(&expected)?;
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        // Deserialize
        assert_eq!(expected, ProverSolution::from_str(expected_string)?);
        assert_eq!(expected, serde_json::from_str(&candidate_string)?);

        Ok(())
    }

    #[test]
    fn test_bincode() -> Result<()> {
        let mut rng = TestRng::default();
        let private_key = PrivateKey::<CurrentNetwork>::new(&mut rng)?;
        let address = Address::try_from(private_key)?;

        // Sample a new prover puzzle solution.
        let partial_solution = PartialSolution::new(address, u64::rand(&mut rng), KZGCommitment(rng.gen()));
        let expected = ProverSolution::new(partial_solution, KZGProof { w: rng.gen(), random_v: None });

        // Serialize
        let expected_bytes = expected.to_bytes_le()?;
        let expected_bytes_with_size_encoding = bincode::serialize(&expected)?;
        assert_eq!(&expected_bytes[..], &expected_bytes_with_size_encoding[8..]);

        // Deserialize
        assert_eq!(expected, ProverSolution::read_le(&expected_bytes[..])?);
        assert_eq!(expected, bincode::deserialize(&expected_bytes_with_size_encoding[..])?);

        Ok(())
    }
}
