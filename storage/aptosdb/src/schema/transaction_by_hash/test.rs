// Copyright (c) Aptos Foundation
// Parts of the project are originally copyright (c) Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::*;
use aptos_schemadb::{schema::fuzzing::assert_encode_decode, test_no_panic_decoding};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_encode_decode(
        hash in any::<HashValue>(),
        version in any::<Version>(),
    ) {
        assert_encode_decode::<TransactionByHashSchema>(&hash, &version);
    }
}

test_no_panic_decoding!(TransactionByHashSchema);
