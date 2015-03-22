#![feature(rustc_private, unsafe_destructor, std_misc)]

//! "Safe"\* bindings to LLVM, via librustc.
//!
//! \* It's still entirely possible to do invalid things with LLVM, such as
//! passing a function `Value` where a scalar is expected. This library cannot
//! reasonably protect you from such misuses; doing so will probably trigger
//! internal asserts in LLVM, terminating the program.
//!
//! # Example
//!
//! Build a function that increments a value in memory and call it.
//! 
//! ```
//! use interleave_jit::{Context, Module, Builder, Position, Value, ExecutionEngine};
//!
//! let ctxt = Context::new();
//! let module = Module::in_context(&ctxt, "incremeters");
//! {
//!     let mut builder = Builder::in_context(&ctxt);
//!     
//!     // void myfunction(i8*);
//!     let target_type = ctxt.int_type(8);
//!     let pointer_type = ctxt.pointer_type(target_type);
//!     let void = ctxt.void_type();
//!     let function_type = ctxt.function_type(void, &[pointer_type], false);
//!     let func = module.add_function("increment_i8", function_type);
//!     
//!     // Create a basic block inside the function we just created
//!     let bb = ctxt.append_bb(func, "entry");
//!     // Generate code at the end of the new basic block
//!     builder.position(Position::EndOf(bb));
//!     
//!     let params = func.function_params().collect::<Vec<_>>();
//!     let load = builder.build_load(params[0]);
//!     let one = Value::const_int(&target_type, 1, false);
//!     let result = builder.build_add(load, one);
//!     builder.build_store(result, params[0]);
//!     builder.build_ret_void();
//! }
//! 
//! let ee = ExecutionEngine::new(module);
//! let the_function = ee.get_function("increment_i8").expect("No function named \"increment_i8\"");
//! let the_function = unsafe {
//!     std::mem::transmute::<extern "C" fn() -> (),
//!                           extern "C" fn(*mut i8) -> ()>(the_function)
//! };
//! let mut the_value: i8 = 0;
//! the_function(&mut the_value as *mut _);
//! assert_eq!(the_value, 1);
//! ```

extern crate rustc;

use rustc::lib::llvm;
use std::ffi::{CString, IntoBytes};
use std::ops::Deref;
use std::marker::PhantomData;

/// The top-level handle to an instance of the LLVM code generator.
pub struct Context {
    llctxt: llvm::ContextRef
}

impl Context {
    /// Create a new LLVM context.
    pub fn new() -> Context {
        unsafe {
            Context {
                llctxt: llvm::LLVMContextCreate()
            }
        }
    }

    /// Get a type representing a pointer to the provided `target` type.
    ///
    /// These pointers are always in address space 0.
    pub fn pointer_type<'a>(&self, target: Type<'a>) -> Type<'a> {
        Type {
            lltype: unsafe {
                llvm::LLVMPointerType(*target, 0)
            },
            ctxt: PhantomData
        }
    }

    /// Get a type representing an integer of `width` bits.
    pub fn int_type<'a>(&'a self, width: u32) -> Type<'a> {
        Type::generic(self, |cx| unsafe {
            llvm::LLVMIntTypeInContext(cx, width)
        })
    }

    /// Get a type representing no value, equivalent to C `void`.
    pub fn void_type<'a>(&'a self) -> Type<'a> {
        Type::generic(self, |cx| unsafe {
            llvm::LLVMVoidTypeInContext(cx)
        })
    }

    /// Get a type representing a function with specified signature.
    ///
    /// The function returns a value of type `returns`, and takes any number of parameters
    /// in `parameters`. Set `vararg` to make the function variadic.
    /// 
    /// Variadic functions require the parameters specified here, and the variadic arguments
    /// must be handled with the `va_arg` instruction and several intrinsic functions, as
    /// described in the [LLVM documentation](http://llvm.org/docs/LangRef.html#int-varargs).
    pub fn function_type<'a>(&'a self, returns: Type<'a>, parameters: &[Type<'a>], vararg: bool) -> Type<'a> {
        let raw_params: Vec<llvm::TypeRef> = parameters.iter().map(|t| t.lltype).collect();
        Type::generic(self, |_| unsafe {
            llvm::LLVMFunctionType(*returns, raw_params.as_ptr(), raw_params.len() as u32,
                                   if vararg { 1 } else { 0 })
        })
    }

    /// Append a `BasicBlock` to the specified function.
    ///
    /// Panics if `name` contains null bytes.
    pub fn append_bb<'a, T: IntoBytes>(&'a self, function: Value<'a>, name: T) -> BasicBlock<'a> {
        let name = CString::new(name).ok().expect("BasicBlock name may not contain null bytes");
        BasicBlock {
            llbb: unsafe {
                llvm::LLVMAppendBasicBlockInContext(self.llctxt, function.llvalue, name.as_ptr())
            },
            ctxt: PhantomData
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            llvm::LLVMContextDispose(self.llctxt);
        }
    }
}

impl Deref for Context {
    type Target = llvm::ContextRef;

    fn deref(&self) -> &llvm::ContextRef {
        &self.llctxt
    }
}

/// The topmost unit of code, containing zero or more functions.
pub struct Module<'a> {
    llmod: llvm::ModuleRef,
    ctxt: PhantomData<&'a Context>
}

impl<'a> Module<'a> {
    /// Create a new `Module` inside a `Context.
    ///
    /// Panics if `name` contains null bytes.
    pub fn in_context<S: IntoBytes>(ctxt: &'a Context, name: S) -> Module<'a> {
        let bstring = CString::new(name).ok().expect("Module name may not contain null bytes");
        unsafe {
            Module {
                llmod: llvm::LLVMModuleCreateWithNameInContext(bstring.as_ptr(),
                                                               **ctxt),
                ctxt: PhantomData
            }
        }
    }

    /// Create a new function inside a module with specified signature.
    ///
    /// See `Context::function_type` for generating `ty`.
    ///
    /// Panics if `name` contains null bytes.
    pub fn add_function<T: IntoBytes>(&self, name: T, ty: Type) -> Value<'a> {
        let name = CString::new(name).ok().expect("Function name may not contain null bytes");
        unsafe {
            Value {
                llvalue: llvm::LLVMAddFunction(self.llmod, name.as_ptr(), *ty),
                ctxt: PhantomData
            }
        }
    }

    /// Write a text representation of a module to standard output.
    ///
    /// Since this is implemented inside LLVM, the output cannot be redirected.
    pub fn dump(&self) {
        unsafe {
            llvm::LLVMDumpModule(**self)
        }
    }
}

impl<'a> Deref for Module<'a> {
    type Target = llvm::ModuleRef;

    fn deref(&self) -> &llvm::ModuleRef {
        &self.llmod
    }
}

#[unsafe_destructor]
impl<'a> Drop for Module<'a> {
    fn drop(&mut self) {
        unsafe {
            llvm::LLVMDisposeModule(self.llmod);
        }
    }
}

/// A type, exactly one of which is associated with any given `Value`.
///
/// See http://www.llvm.org/docs/doxygen/html/classllvm_1_1Type.html
pub struct Type<'a> {
    lltype: llvm::TypeRef,
    ctxt: PhantomData<&'a Context>
}

impl<'a> Type<'a> {
    fn generic<'b, F: FnOnce(llvm::ContextRef) -> llvm::TypeRef>(ctxt: &'b Context, f: F) -> Type<'b> {
        Type {
            lltype: f(ctxt.llctxt),
            ctxt: PhantomData
        }
    }
}

impl<'a> Copy for Type<'a> { }

impl<'a> Deref for Type<'a> {
    type Target = llvm::TypeRef;

    fn deref(&self) -> &llvm::TypeRef {
        &self.lltype
    }
}

/// A value computed by the program, usable as an operand.
///
/// See http://llvm.org/docs/doxygen/html/classllvm_1_1Value.html
pub struct Value<'a> {
    llvalue: llvm::ValueRef,
    ctxt: PhantomData<&'a Context>
}

impl<'a> Copy for Value<'a> { }

impl<'a> Deref for Value<'a> {
    type Target = llvm::ValueRef;

    fn deref(&self) -> &llvm::ValueRef {
        &self.llvalue
    }
}

/// Iterator over parameters to a `Function` as `Value`s.
///
/// You should not construct these manually; use `Value::function_params` instead.
pub enum FunctionParameters<'a> {
    Initial(Value<'a>),     // Function
    Secondary(Value<'a>),   // Parameter
    Done
}

impl<'a> Iterator for FunctionParameters<'a> {
    type Item = Value<'a>;

    fn next(&mut self) -> Option<Value<'a>> {
        use FunctionParameters::*;

        let next_arg = unsafe {
            match self {
                &mut Initial(ref func) => {
                    llvm::LLVMGetFirstParam(**func)
                }
                &mut Secondary(ref param) => {
                    llvm::LLVMGetNextParam(**param)
                }
                &mut Done => return None
            }
        };
        if next_arg.is_null() {
            *self = Done;
            None
        } else {
            let v = Value {
                llvalue: next_arg,
                ctxt: PhantomData
            };
            *self = Secondary(v);
            Some(v)
        }
    }
}

impl<'a> Value<'a> {
    fn build<'b>(ptr: llvm::ValueRef) -> Value<'b> {
        Value {
            llvalue: ptr,
            ctxt: PhantomData
        }
    }

    /// Get an iterator over parameters to a `Function`.
    ///
    /// This `Value` must be an actual `Function`.
    pub fn function_params(&self) -> FunctionParameters<'a> {
        FunctionParameters::Initial(*self)
    }

    /// Get a contant integer value of specified type (as from `Context::int_type`).
    pub fn const_int(ty: &Type<'a>, value: u64, signed: bool) -> Value<'a> {
        unsafe {
            Value::build(
                llvm::LLVMConstInt(**ty, value, if signed { 1 } else { 0 })
            )
        }
    }
}

/// A block of code with exactly one entry point.
///
/// See http://llvm.org/doxygen/classllvm_1_1BasicBlock.html
pub struct BasicBlock<'a> {
    llbb: llvm::BasicBlockRef,
    ctxt: PhantomData<&'a Context>
}

impl<'a> Copy for BasicBlock<'a> { }

impl<'a> Deref for BasicBlock<'a> {
    type Target = llvm::BasicBlockRef;

    fn deref(&self) -> &llvm::BasicBlockRef {
        &self.llbb
    }
}

/// Builds new `Value`s by inserting instructions into `BasicBlock`s.
///
/// After creating a new `Builder`, it must be `position`ed before code can
/// be generated.
///
/// See http://www.llvm.org/docs/doxygen/html/classllvm_1_1IRBuilder.html
pub struct Builder<'a> {
    llbld: llvm::BuilderRef,
    next_iv: usize,
    ctxt: PhantomData<&'a Context>
}

/// Place to position a `Builder`, as in `Builder::position`.
pub enum Position<'a> {
    /// At the end of the given `BasicBlock`.
    EndOf(BasicBlock<'a>),
    /// Immediately preceding the producer of `Value`, which must be an Instruction.
    ///
    /// Presumably this only seeks within the current basic block.
    Before(Value<'a>),
    /// At (presumably immediately following) a given Value in a specified basic block.
    At(BasicBlock<'a>, Value<'a>)
}

impl<'a> Builder<'a> {
    fn get_name(&mut self) -> CString {
        let iv = self.next_iv;
        self.next_iv += 1;
        CString::new(format!("t{}", iv)).unwrap()
    }

    /// Construct a `Builder` to work inside the given `Context`.
    pub fn in_context(ctxt: &'a Context) -> Builder<'a> {
        Builder {
            llbld: unsafe { llvm::LLVMCreateBuilderInContext(ctxt.llctxt) },
            next_iv: 0,
            ctxt: PhantomData
        }
    }

    /// Position a builder's output pointer.
    pub fn position(&self, pos: Position) {
        unsafe {
            match pos {
                Position::EndOf(b) => llvm::LLVMPositionBuilderAtEnd(**self, *b),
                Position::Before(v) => llvm::LLVMPositionBuilderBefore(**self, *v),
                Position::At(b, v) => llvm::LLVMPositionBuilder(**self, *b, *v)
            }
        }
    }

    /// Build a `ret void`, for returning from a function declared to return `void`.
    pub fn build_ret_void(&self) -> Value<'a> {
        Value {
            llvalue: unsafe {
                llvm::LLVMBuildRetVoid(**self)
            },
            ctxt: PhantomData
        }
    }

    /// Build a load from memory, yielding the value pointed to by `ptr`.
    pub fn build_load(&mut self, ptr: Value<'a>) -> Value<'a> {
        unsafe {
            Value::build(llvm::LLVMBuildLoad(**self, *ptr, self.get_name().as_ptr()))
        }
    }

    /// Build a store to memory, writing `value` to memory pointed to by `ptr`.
    ///
    /// Yields `void` because a store has no output.
    pub fn build_store(&self, value: Value<'a>, ptr: Value<'a>) -> Value<'a> {
        unsafe {
            Value::build(llvm::LLVMBuildStore(**self, *value, *ptr))
        }
    }

    /// Build an addition of two integer values, yielding their sum.
    ///
    /// Works only for integers and vectors of integers. For floating-point values,
    /// use `build_fadd`. Pointers must first be cast to integers then mutated
    /// and cast back.
    pub fn build_add(&mut self, lhs: Value<'a>, rhs: Value<'a>) -> Value<'a> {
        unsafe {
            Value::build(llvm::LLVMBuildAdd(**self, *lhs, *rhs, self.get_name().as_ptr()))
        }
    }
}

impl<'a> Deref for Builder<'a> {
    type Target = llvm::BuilderRef;

    fn deref(&self) -> &llvm::BuilderRef {
        &self.llbld
    }
}

#[unsafe_destructor]
impl<'a> Drop for Builder<'a> {
    fn drop(&mut self) {
        unsafe {
            llvm::LLVMDisposeBuilder(self.llbld);
        }
    }
}

/// Runtime code generator.
///
/// When given a `Module`, the `ExecutionEngine` will JIT-compile the module and
/// allow you to execute the generated code.
pub struct ExecutionEngine<'a> {
    llee: llvm::ExecutionEngineRef,
    module: PhantomData<Module<'a>>
}

impl<'a> ExecutionEngine<'a> {
    /// Construct a new execution engine with the code in the given module.
    pub fn new(module: Module<'a>) -> ExecutionEngine<'a> {
        unsafe {
            // XXX it's not clear if the execution engine will ever refer to __morestack
            // in the default configuration. I assume it won't unless we specifically ask
            // it for split stacks.
            // This will be a really obvious crash if it tries to call NULL at least.
            let mm = llvm::LLVMRustCreateJITMemoryManager(std::ptr::null());

            // Module ownership moves into the ExecutionEngine apparently
            let llmod = *module;
            std::mem::forget(module);

            ExecutionEngine {
                llee: llvm::LLVMBuildExecutionEngine(llmod, mm),
                module: PhantomData
            }
        }
    }

    /// Get a pointer to the function with the given name, if any.
    ///
    /// The type of the returned value is a function pointer, which unfortunately
    /// cannot be made generic because there's no way to enforce that a given type
    /// parameter is pointer-width (or, more specifically, is a non-Rust-ABI function
    /// pointer).
    ///
    /// Thus, you'll need to `transmute` the output to match your expected function
    /// signature.
    pub fn get_function<T: IntoBytes>(&self, name: T) -> Option<extern "C" fn() -> ()> {
        let cs = CString::new(name).unwrap();
        unsafe {
            let p = LLVMGetFunctionAddress(**self, cs.as_ptr());
            if p.is_null() {
                None
            } else {
                Some(std::mem::transmute(p))
            }
        }
    }
}

impl<'a> Deref for ExecutionEngine<'a> {
    type Target = llvm::ExecutionEngineRef;

    fn deref(&self) -> &llvm::ExecutionEngineRef {
        &self.llee
    }
}

#[unsafe_destructor]
impl<'a> Drop for ExecutionEngine<'a> {
    fn drop(&mut self) {
        unsafe {
            llvm::LLVMDisposeExecutionEngine(self.llee);
        }
    }
}

// This is probably very wrong, not in the least because rustc uses its own LLVM and we'll
// be using the system one.
//
// The "proper" approach here is probably to simply not use librustc and go directly
// through LLVM-C.
//
// See `llvm-config --libs engine`.
#[link(name="LLVMMC")]
extern "C" {
    fn LLVMGetFunctionAddress(ee: llvm::ExecutionEngineRef, name: *const i8) -> *const ();
}
