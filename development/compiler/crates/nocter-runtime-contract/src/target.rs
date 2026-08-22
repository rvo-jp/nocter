/// One compiler-granted runtime ABI. Consumers may select layouts from this identity but cannot
/// construct or infer target capability from source/package spellings.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeAbiIdentity {
    Arm64DarwinV1,
}
