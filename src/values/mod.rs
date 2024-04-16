mod heap_value;
mod functions;
mod lazy_iter;
mod object;
mod tagged_value;
mod value;

pub use value::Value;
pub use heap_value::HeapValue;
pub use functions::*;
pub use lazy_iter::*;
pub use object::*;
pub use tagged_value::TaggedValue;

#[derive(Debug)]
pub enum ReturnValue {
    Value(value::Value),
    HeapValue(heap_value::HeapValue),
}