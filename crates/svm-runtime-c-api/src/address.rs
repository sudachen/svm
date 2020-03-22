use crate::{impl_from_svm_byte_array, impl_into_svm_byte_array};

use svm_common::Address;

impl_from_svm_byte_array!(Address);
impl_into_svm_byte_array!(Address);
