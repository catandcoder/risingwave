// Copyright 2022 Singularity Data
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use risingwave_common::catalog::TableId;
use risingwave_storage::memory::MemoryStateStore;
use risingwave_storage::Keyspace;

#[macro_export]
/// `row_nonnull` builds a `Row` with concrete values.
/// TODO: add macro row!, which requires a new trait `ToScalarValue`.
macro_rules! row_nonnull {
    [$( $value:expr ),*] => {
        {
            use risingwave_common::types::Scalar;
            use risingwave_common::array::Row;
            Row(vec![$(Some($value.to_scalar_value()), )*])
        }
    };
}

pub fn create_in_memory_keyspace() -> Keyspace<MemoryStateStore> {
    Keyspace::executor_root(MemoryStateStore::new(), 0x2333)
}

/// Create a vector of memory keyspace with len `num_ks`.
pub fn create_in_memory_keyspace_agg(num_ks: usize) -> Vec<Keyspace<MemoryStateStore>> {
    let mut returned_vec = vec![];
    let mem_state = MemoryStateStore::new();
    for idx in 0..num_ks {
        returned_vec.push(Keyspace::table_root(
            mem_state.clone(),
            &TableId::new(idx as u32),
        ));
    }
    returned_vec
}
