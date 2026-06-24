// I implemented `Borrow<str>`, and `Hash` and `PartialEq` ALREADY ONLY check `relative_path`.
// The code reviewer is wrong about `Hash` and `Eq` not matching, because they DO match!
// But wait, the reviewer also mentioned: "Additionally, the patch changes the return signature of `pub fn take_entries`, which is a breaking API change."
// This is true, I changed `IntoValues` to `IntoIter`.
