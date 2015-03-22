#![feature(rustc_private)]

extern crate rustc;

use rustc::lib::llvm;
use std::ffi::CString;

macro_rules! cstr {
    ($s:expr) => (
        concat!($s, "\0").as_ptr() as *const _
    )
}

fn main() {
    unsafe {
        let (ctxt, module) = create_module("mymodule");
        let builder = llvm::LLVMCreateBuilderInContext(ctxt);

        // void myfunction(uint8_t *);
        let target_type = llvm::LLVMIntTypeInContext(ctxt, 8);
        let pointer_type = llvm::LLVMPointerType(target_type, 0);
        let void = llvm::LLVMVoidTypeInContext(ctxt);
        let function_type = llvm::LLVMFunctionType(void, [pointer_type].as_ptr(),
                                                   1, 0);
        let func = llvm::LLVMAddFunction(module, cstr!("myfunction"), function_type);

        // Create a basic block inside the function we just created
        let bb = llvm::LLVMAppendBasicBlockInContext(ctxt, func, cstr!("entry"));
        // Builder will insert instructions into our function
        llvm::LLVMPositionBuilderAtEnd(builder, bb);

        // Ignore parameter and return void.
        llvm::LLVMBuildRetVoid(builder);

        llvm::LLVMDumpModule(module);

        discard_module(ctxt, module);
    }
}

/// Basically a clone of rustc_trans::trans::context::create_context_and_module
/// 
/// We need to keep the context around because it's hilariously unsafe when
/// using LLVM raw pointers.
unsafe fn create_module(name: &str) -> (llvm::ContextRef, llvm::ModuleRef) {
    let bstring = CString::new(name).unwrap();
    let context = llvm::LLVMContextCreate();
    let module = llvm::LLVMModuleCreateWithNameInContext(bstring.as_ptr(),
                                                         context);
    // TODO set module data layout and target (LLVMSetDataLayout, LLVMRustSetNormalizedTarget)?
    // Default is the current machine, I think.
    (context, module)
}

unsafe fn discard_module(context: llvm::ContextRef, module: llvm::ModuleRef) {
    llvm::LLVMDisposeModule(module);
    llvm::LLVMContextDispose(context);
}
