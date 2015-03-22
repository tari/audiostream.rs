extern crate "interleave_jit" as ellell;

use ellell::{Context, Module, Builder, Position, ExecutionEngine, Value};

fn main() {
    let ctxt = Context::new();
    let module = Module::in_context(&ctxt, "incremeters");
    {
        let mut builder = Builder::in_context(&ctxt);
        
        // void myfunction(i8*);
        let target_type = ctxt.int_type(8);
        let pointer_type = ctxt.pointer_type(target_type);
        let void = ctxt.void_type();
        let function_type = ctxt.function_type(void, &[pointer_type], false);
        let func = module.add_function("increment_i8", function_type);
        
        // Create a basic block inside the function we just created
        let bb = ctxt.append_bb(func, "entry");
        // Generate code at the end of the new basic block
        builder.position(Position::EndOf(bb));
        
        let params = func.function_params().collect::<Vec<_>>();
        let load = builder.build_load(params[0]);
        let one = Value::const_int(&target_type, 1, false);
        let result = builder.build_add(load, one);
        builder.build_store(result, params[0]);
        builder.build_ret_void();
        module.dump();
    }

    let ee = ExecutionEngine::new(module);
    let the_function = ee.get_function("increment_i8").expect("No function named \"increment_i8\"");
    let the_function = unsafe {
        std::mem::transmute::<extern "C" fn() -> (),
                              extern "C" fn(*mut i8) -> ()>(the_function)
    };
    println!("&increment_i8 = {:?}", the_function as *const ());

    let mut the_value: i8 = 0;
    println!("x = {}", the_value);
    println!("increment_i8(&x)");
    the_function(&mut the_value as *mut _);
    println!("x = {}", the_value);
    assert_eq!(the_value, 1);
}
