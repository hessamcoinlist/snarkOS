use crate::crh::{PedersenCRHParameters, PedersenSize};
use snarkos_errors::{algorithms::CRHError, curves::ConstraintFieldError};
use snarkos_models::{
    algorithms::CRH,
    curves::{to_field_vec::ToConstraintField, Field, Group},
};

use rand::Rng;

#[cfg(feature = "pedersen-parallel")]
use rayon::prelude::*;

pub fn bytes_to_bits(bytes: &[u8]) -> Vec<bool> {
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    for byte in bytes {
        for i in 0..8 {
            let bit = (*byte >> i) & 1;
            bits.push(bit == 1)
        }
    }
    bits
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PedersenCRH<G: Group, S: PedersenSize> {
    pub parameters: PedersenCRHParameters<G, S>,
}

impl<G: Group, S: PedersenSize> CRH for PedersenCRH<G, S> {
    type Output = G;
    type Parameters = PedersenCRHParameters<G, S>;

    const INPUT_SIZE_BITS: usize = S::WINDOW_SIZE * S::NUM_WINDOWS;

    fn setup<R: Rng>(rng: &mut R) -> Self {
        Self {
            parameters: PedersenCRHParameters::setup(rng),
        }
    }

    fn hash(&self, input: &[u8]) -> Result<Self::Output, CRHError> {
        if (input.len() * 8) > S::WINDOW_SIZE * S::NUM_WINDOWS {
            return Err(CRHError::IncorrectInputLength(
                input.len(),
                S::WINDOW_SIZE,
                S::NUM_WINDOWS,
            ));
        }

        // Pad the input if it is not the current length.
        let mut input = input;
        let mut padded_input = vec![];
        if (input.len() * 8) < S::WINDOW_SIZE * S::NUM_WINDOWS {
            padded_input.extend_from_slice(input);
            for _ in input.len()..((S::WINDOW_SIZE * S::NUM_WINDOWS) / 8) {
                padded_input.push(0u8);
            }
            input = padded_input.as_slice();
        }

        if self.parameters.bases.len() != S::NUM_WINDOWS {
            return Err(CRHError::IncorrectParameterSize(
                self.parameters.bases[0].len(),
                self.parameters.bases.len(),
                S::WINDOW_SIZE,
                S::NUM_WINDOWS,
            ));
        }

        // Compute sum of h_i^{m_i} for all i.
        let result = {
            #[cfg(feature = "pedersen-parallel")]
            {
                bytes_to_bits(input)
                    .par_chunks(S::WINDOW_SIZE)
                    .zip(&self.parameters.bases)
                    .map(|(bits, powers)| {
                        let mut encoded = G::zero();
                        for (bit, base) in bits.iter().zip(powers.iter()) {
                            if *bit {
                                encoded += base;
                            }
                        }
                        encoded
                    })
                    .reduce(G::zero, |a, b| a + &b)
            }
            #[cfg(not(feature = "pedersen-parallel"))]
            {
                bytes_to_bits(input)
                    .chunks(S::WINDOW_SIZE)
                    .zip(&self.parameters.bases)
                    .map(|(bits, powers)| {
                        let mut encoded = G::zero();
                        for (bit, base) in bits.iter().zip(powers.iter()) {
                            if *bit {
                                encoded += base;
                            }
                        }
                        encoded
                    })
                    .fold(G::zero(), |a, b| a + &b)
            }
        };

        Ok(result)
    }

    fn parameters(&self) -> &Self::Parameters {
        &self.parameters
    }
}

impl<G: Group, S: PedersenSize> From<PedersenCRHParameters<G, S>> for PedersenCRH<G, S> {
    fn from(parameters: PedersenCRHParameters<G, S>) -> Self {
        Self { parameters }
    }
}

impl<F: Field, G: Group + ToConstraintField<F>, S: PedersenSize> ToConstraintField<F> for PedersenCRH<G, S> {
    #[inline]
    fn to_field_elements(&self) -> Result<Vec<F>, ConstraintFieldError> {
        self.parameters.to_field_elements()
    }
}
