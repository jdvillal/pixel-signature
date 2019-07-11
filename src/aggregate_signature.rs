use rand::{CryptoRng, RngCore};

use crate::amcl_wrapper::group_elem::GroupElement;
use crate::errors::PixelError;
use crate::keys::{GeneratorSet, Verkey};
use crate::signature::Signature;
use amcl_wrapper::extension_field_gt::GT;
use amcl_wrapper::group_elem_g1::G1;
use amcl_wrapper::group_elem_g2::G2;

pub struct AggregatedVerkey {
    pub value: G2,
}

impl AggregatedVerkey {
    pub fn new(ver_keys: Vec<&Verkey>) -> Self {
        let mut avk: G2 = G2::identity();
        for vk in ver_keys {
            avk += vk.value;
        }
        AggregatedVerkey { value: avk }
    }

    pub fn is_identity(&self) -> bool {
        if self.value.is_identity() {
            println!("AggregatedVerkey point at infinity");
            return true;
        }
        return false;
    }
}

pub struct AggregatedSignature {
    pub sigma_1: G1,
    pub sigma_2: G2,
}

// TODO: Merge with signature and remove duplicate code
impl AggregatedSignature {
    pub fn new(sigs: Vec<&Signature>) -> Self {
        let mut asig_1 = G1::identity();
        let mut asig_2 = G2::identity();
        for s in sigs {
            asig_1 += s.sigma_1;
            asig_2 += s.sigma_2;
        }
        AggregatedSignature {
            sigma_1: asig_1,
            sigma_2: asig_2,
        }
    }

    pub fn is_identity(&self) -> bool {
        if self.sigma_1.is_identity() {
            println!("Signature point in G1 at infinity");
            return true;
        }
        if self.sigma_2.is_identity() {
            println!("Signature point in G2 at infinity");
            return true;
        }
        return false;
    }

    pub fn has_correct_oder(&self) -> bool {
        if !self.sigma_1.has_correct_order() {
            println!("Signature point in G1 has incorrect order");
            return false;
        }
        if !self.sigma_2.has_correct_order() {
            println!("Signature point in G2 has incorrect order");
            return false;
        }
        return true;
    }

    pub fn verify(
        &self,
        msg: &[u8],
        t: u128,
        l: u8,
        ver_keys: Vec<&Verkey>,
        gens: &GeneratorSet,
    ) -> Result<bool, PixelError> {
        let avk = AggregatedVerkey::new(ver_keys);
        self.verify_using_aggr_vk(msg, t, l, &avk, gens)
    }

    // For verifying multiple aggregate signatures from the same group of signers,
    // an aggregated verkey should be created once and then used for each signature verification
    pub fn verify_using_aggr_vk(
        &self,
        msg: &[u8],
        t: u128,
        l: u8,
        avk: &AggregatedVerkey,
        gens: &GeneratorSet,
    ) -> Result<bool, PixelError> {
        if self.is_identity() || avk.is_identity() || !self.has_correct_oder() {
            return Ok(false);
        }
        if gens.1.len() < (l as usize + 2) {
            return Err(PixelError::NotEnoughGenerators { n: l as usize + 2 });
        }
        Signature::verify_naked(&self.sigma_1, &self.sigma_2, &avk.value, msg, t, l, gens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{setup, Keypair, Sigkey, SigkeySet};
    use crate::util::calculate_l;
    use rand::rngs::ThreadRng;
    use std::process::abort;

    pub fn create_sig_and_verify<R: RngCore + CryptoRng>(
        set: &SigkeySet,
        t: u128,
        vk: &Verkey,
        l: u8,
        gens: &GeneratorSet,
        mut rng: &mut R,
    ) {
        let sk = set.get_key(t).unwrap();
        let msg = "Hello".as_bytes();
        let sig = Signature::new(msg, t, l, &gens, &sk, &mut rng).unwrap();
        assert!(sig.verify(msg, t, l, &gens, &vk).unwrap());
    }

    #[test]
    fn test_aggr_sig_verify() {
        let mut rng = rand::thread_rng();
        let T = 7;
        let l = calculate_l(T).unwrap();
        let mut t = 1u128;

        let (gens, vk1, mut sigkey_set1, _) =
            setup::<ThreadRng>(T, "test_pixel", &mut rng).unwrap();

        let (keypair2, mut sigkey_set2) = Keypair::new(T, &gens, &mut rng).unwrap();
        let vk2 = keypair2.ver_key;

        create_sig_and_verify::<ThreadRng>(&sigkey_set1, t, &vk1, l, &gens, &mut rng);
        create_sig_and_verify::<ThreadRng>(&sigkey_set2, t, &vk2, l, &gens, &mut rng);

        {
            let msg = "Hello".as_bytes();
            let sk1 = sigkey_set1.get_key(t).unwrap();
            let sig1 = Signature::new(msg, t, l, &gens, &sk1, &mut rng).unwrap();
            let sk2 = sigkey_set2.get_key(t).unwrap();
            let sig2 = Signature::new(msg, t, l, &gens, &sk2, &mut rng).unwrap();

            let asig = AggregatedSignature::new(vec![&sig1, &sig2]);
            assert!(asig.verify(msg, t, l, vec![&vk1, &vk2], &gens).unwrap());
        }

        {
            t = 3;
            sigkey_set1.fast_forward_update(t, &gens, &mut rng).unwrap();
            sigkey_set2.fast_forward_update(t, &gens, &mut rng).unwrap();

            let msg = "Hello".as_bytes();
            let sk1 = sigkey_set1.get_key(t).unwrap();
            let sig1 = Signature::new(msg, t, l, &gens, &sk1, &mut rng).unwrap();
            let sk2 = sigkey_set2.get_key(t).unwrap();
            let sig2 = Signature::new(msg, t, l, &gens, &sk2, &mut rng).unwrap();

            let asig = AggregatedSignature::new(vec![&sig1, &sig2]);
            assert!(asig.verify(msg, t, l, vec![&vk1, &vk2], &gens).unwrap());
        }

        {
            t = 5;
            sigkey_set1.fast_forward_update(t, &gens, &mut rng).unwrap();
            sigkey_set2.fast_forward_update(t, &gens, &mut rng).unwrap();

            let msg = "Hello".as_bytes();
            let sk1 = sigkey_set1.get_key(t).unwrap();
            let sig1 = Signature::new(msg, t, l, &gens, &sk1, &mut rng).unwrap();
            let sk2 = sigkey_set2.get_key(t).unwrap();
            let sig2 = Signature::new(msg, t, l, &gens, &sk2, &mut rng).unwrap();

            let asig = AggregatedSignature::new(vec![&sig1, &sig2]);
            assert!(asig.verify(msg, t, l, vec![&vk1, &vk2], &gens).unwrap());
        }
    }
}
