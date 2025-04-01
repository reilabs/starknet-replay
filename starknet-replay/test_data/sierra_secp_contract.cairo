#[starknet::contract]
mod TestContract {
    use starknet::{secp256r1::Secp256r1Impl, SyscallResultTrait};
    use starknet::secp256_trait::{recover_public_key, Secp256PointTrait, Signature, is_valid_signature};
    use starknet::secp256r1::{Secp256r1Point, Secp256r1PointImpl};
    #[storage]
    struct Storage {
    }
    #[external(v0)]
    fn test_secp256k1(ref self: ContractState) {
        let (public_key_x, public_key_y) = (
            0x04aaec73635726f213fb8a9e64da3b8632e41495a944d0045b522eba7240fad5,
            0x0087d9315798aaa3a5ba01775787ced05eaaf7b4e09fc81d6d1aa546e8365d525d
        );

        let public_key_a = Secp256r1Impl::secp256_ec_new_syscall(public_key_x, public_key_y)
            .unwrap_syscall()
            .unwrap();

        public_key_a.mul(3).unwrap_syscall();
    }
}
