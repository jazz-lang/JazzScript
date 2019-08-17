pub mod opcodes;
#[macro_use]
pub mod runtime;
pub mod codegen;
pub mod value;
//////cgc::generational::*;
use crate::str;
use opcodes::Opcode;
use value::*;
pub fn nil() -> Value {
    new_ref(ValueData::Nil)
}
use crate::map::LinkedHashMap;
use crate::token::Position;

pub struct Machine {
    pub constants: Ref<Vec<ValueData>>,
    pub line_no: LinkedHashMap<(usize, Opcode), Position>,
}

impl Machine {
    pub fn new() -> Machine {
        Machine {
            constants: new_ref(vec![]),
            line_no: LinkedHashMap::new(),
        }
    }
}

#[derive(Debug)]
enum ExecData {
    Pc(usize),
    Env(Environment),
    Code(Ref<Vec<Opcode>>),
    Stack(Vec<Value>),
    C(Ref<Vec<ValueData>>),
}

/*impl //cgc::Collectable for ExecData {
    fn child(&self) -> Vec<//cgc::GCValue<dyn //cgc::Collectable>> {
        let mut v: Vec<//cgc::GCValue<dyn //cgc::Collectable>> = vec![];
        match self {
            ExecData::Env(x) => {
                v.push(x.gc());
            }
            ExecData::Code(c) => {
                v.push(c.gc());
            }
            ExecData::Stack(s) => {
                for x in s.iter() {
                    v.push(x.gc());
                }
            }
            ExecData::C(s) => {
                v.push(s.gc());
            }
            _ => ()
        }
        v
    }
}
*/
pub struct Frame<'a> {
    pub m: &'a mut Machine,
    pub code: crate::vm::value::Ref<Vec<Opcode>>,
    pub stack: Vec<Value>,
    pub pc: usize,
    pub env: Environment,
    pub funs: Vec<Ref<Function>>,
    exec_stack: Ref<Vec<ExecData>>,
    exception_stack: Vec<usize>,
    cur_ins: Opcode,
}

impl<'a> Frame<'a> {
    pub fn new(m: &'a mut Machine) -> Frame<'a> {
        let f = Frame {
            m,
            code: new_ref(vec![]),
            stack: vec![],
            pc: 0,
            env: new_object(),
            funs: vec![],
            cur_ins: Opcode::BlockEnd,
            exec_stack: new_ref(vec![]),
            exception_stack: vec![],
        };
        f
    }

    pub fn restore_state(
        &mut self,
        _restore_pc: bool,
        _restore_env: bool,
        _restore_code: bool,
        _restore_stack: bool,
    ) {
        let val = self.exec_stack.borrow_mut().pop();
        if let Some(ExecData::C(s)) = val {
            self.m.constants = s
        } else {
            panic!()
        };
        //if restore_pc {
        if let Some(ExecData::Pc(pc)) = self.exec_stack.borrow_mut().pop() {
            self.pc = pc;
        } else {
            panic!()
        }
        //}
        //if restore_env {
        let val = self.exec_stack.borrow_mut().pop();
        if let Some(ExecData::Env(env)) = val {
            self.env = env;
        } else {
            panic!("{:?}", val)
        }
        //}
        //if restore_code {
        if let Some(ExecData::Code(code)) = self.exec_stack.borrow_mut().pop() {
            self.code = code;
        } else {
            panic!()
        }
        //}
        //if restore_stack {
        if let Some(ExecData::Stack(stack)) = self.exec_stack.borrow_mut().pop() {
            self.stack = stack;
        } else {
            panic!()
        }
        //}
    }

    pub fn save_state(
        &mut self,
        _save_pc: bool,
        _save_env: bool,
        _save_code: bool,
        _save_stack: bool,
    ) {
        //if save_stack {
        self.exec_stack
            .borrow_mut()
            .push(ExecData::Stack(self.stack.clone()));
        //}

        self.exec_stack
            .borrow_mut()
            .push(ExecData::Code(self.code.clone()));
        //}
        //if save_env {
        self.exec_stack
            .borrow_mut()
            .push(ExecData::Env(self.env.clone()));
        //}
        //if save_pc {
        self.exec_stack.borrow_mut().push(ExecData::Pc(self.pc));
        //}
        self.exec_stack
            .borrow_mut()
            .push(ExecData::C(self.m.constants.clone()));
    }

    pub fn push_env(&mut self) {
        let old_env = self.env.clone();
        self.env = new_ref(Object {
            proto: Some(old_env),
            table: crate::vm::value::PropertyMap::new(),
        });
        //gc_add_root(self.env.gc());
        //crate::gc:://gc::new_ref(self.env,old_env);
    }

    pub fn pop_env(&mut self) {
        if self.env.borrow().proto.is_none() {
            panic!("No env to pop");
        }
        //gc_rmroot(self.env.gc());

        let proto = {
            let env = self.env.borrow();
            env.proto.as_ref().unwrap().clone()
        };
        self.env = proto.clone();
    }

    pub fn push(&mut self, val: impl Into<ValueData>) {
        self.stack.push(new_ref(val.into()));
    }

    pub fn push_ref(&mut self, val: Value) {
        self.stack.push(val);
    }
    pub fn get_pos(&self) -> Position {
        let pos = self
            .m
            .line_no
            .get(&(self.pc - 1, self.code.borrow()[self.pc - 1]))
            .unwrap()
            .clone();

        pos
    }
    pub fn pop(&mut self) -> Result<Value, ValueData> {
        match self.stack.pop() {
            Some(val) => Ok(val),
            None => Err(new_error(
                -1,
                None,
                &format!("Stack empty. Current instruction: {:?}", self.cur_ins),
            )),
        }
    }

    pub fn execute(&mut self) {
        macro_rules! catch {
            ($result: expr) => {
                match $result {
                    Ok(val) => val,
                    Err(e) => {
                        if let Some(location) = self.exception_stack.pop() {
                            self.pc = location;
                            self.push(e);
                            continue;
                        } else {
                            eprintln!("{}: {}", line!(), e);
                            std::process::exit(1);
                        }
                    }
                }
            };
        }

        macro_rules! throw {
            ($msg: expr) => {{
                let error = new_error(-1, None, &format!("Runtime exception: {}", $msg));
                if let Some(location) = self.exception_stack.pop() {
                    self.pc = location;
                    self.push(error);
                    continue;
                } else {
                    eprintln!("{}", error);
                    std::process::exit(-1);
                }
            }};
        }

        while self.pc < self.code.borrow().len() {
            let opcode = self.code.borrow()[self.pc];
            self.cur_ins = opcode;
            self.pc += 1;
            use Opcode::*;
            match opcode {
                NewIter => {
                    let value = catch!(self.pop());
                    let mut values = vec![];
                    let value: &ValueData = &value.borrow();
                    match value {
                        ValueData::Object(object) => {
                            for (key, val) in object.borrow().table.iter() {
                                let entry = new_object();
                                entry.borrow_mut().set("key", new_ref(key.clone())).unwrap();
                                entry.borrow_mut().set("value", val.clone()).unwrap();
                                values.push(new_ref(ValueData::Object(entry)));
                            }
                        }
                        ValueData::Array(values_) => {
                            for val in values_.borrow().iter() {
                                values.push(val.clone());
                            }
                        }
                        ValueData::Iterator(iterator) => {
                            self.stack
                                .push(new_ref(ValueData::Iterator(iterator.clone())));
                            continue;
                        }
                        _ => throw!("Array or object expected in iterator instance"),
                    }
                    let iter = new_ref(ValueIter { values });
                    //gc_add_root(iter);

                    self.stack.push(new_ref(ValueData::Iterator(iter)));
                }
                IterHasNext => {
                    let maybe_iter = catch!(self.pop());
                    let maybe_iter: &ValueData = &maybe_iter.borrow();
                    match maybe_iter {
                        ValueData::Iterator(iter) => {
                            self.stack
                                .push(new_ref(ValueData::Bool(iter.borrow().has_next())));
                        }
                        x => panic!("{:?}", x),
                    }
                }
                IterNext => {
                    let maybe_iter = catch!(self.pop());
                    let maybe_iter: &ValueData = &maybe_iter.borrow();
                    match maybe_iter {
                        ValueData::Iterator(iter) => {
                            self.stack.push(iter.borrow_mut().next());
                        }
                        _ => unreachable!(),
                    }
                }
                LoadConst(index) => {
                    let constant = self.m.constants.borrow()[index as usize].clone();
                    self.push(constant);
                }
                LoadInt(val) => {
                    self.push(val);
                }
                LoadFalse => {
                    self.push(false);
                }
                LoadTrue => {
                    self.push(true);
                }

                LoadNil => {
                    self.push(ValueData::Nil);
                }
                LoadUndef => {
                    self.push(ValueData::Undefined);
                }
                LoadVar(var) => {
                    //let pos = *self.m.line_no.get(&(self.pc, opcode)).unwrap();
                    let pos = Position::new(0, 0);

                    let variable = catch!(get_variable(
                        &self.env,
                        ValueData::String(str(var).to_string()),
                        &pos
                    ));
                    self.push_ref(variable);
                }
                DeclVar(name) => {
                    //println!("{} {:#?}",self.pc,self.m.line_no);
                    //let pos = *self.m.line_no.get(&(self.pc - 1, opcode)).unwrap();
                    //
                    let pos = Position::new(0, 0);
                    let val = catch!(self.pop());
                    if var_declared(&self.env, ValueData::String(str(name).to_string())) {
                        catch!(set_variable_in_scope(
                            &self.env,
                            ValueData::String(str(name).to_string()),
                            val,
                            &pos
                        ));
                    } else {
                        catch!(declare_var(
                            &self.env,
                            ValueData::String(str(name).to_string()),
                            val,
                            &pos
                        ));
                    }
                }
                StoreVar(name) => {
                    //let pos = *self.m.line_no.get(&(self.pc - 1, opcode)).unwrap();
                    let pos = Position::new(0, 0);
                    let val = catch!(self.pop());

                    catch!(set_variable_in_scope(
                        &self.env,
                        ValueData::String(str(name).to_string()),
                        val,
                        &pos
                    ));
                }
                Opcode::Dup => {
                    let val = self.stack.pop().unwrap_or(new_ref(ValueData::Undefined));
                    self.push_ref(val.clone());
                    self.push_ref(val);
                }
                ConstructArray(n) => {
                    let array = new_ref(vec![]);
                    for _ in 0..n {
                        let val = catch!(self.pop());
                        array.borrow_mut().push(val);
                        //crate::gc:://gc::new_ref(array,val);
                    }
                    self.push(ValueData::Array(array));
                }
                Store => {
                    let value = catch!(self.pop());
                    let key = catch!(self.pop());
                    let object = catch!(self.pop());

                    catch!(object
                        .borrow_mut()
                        .set(key.borrow().clone(), value.borrow().clone()));
                }
                NewObj => {
                    self.push(ValueData::Object(new_object()));
                }
                Load => {
                    let key = catch!(self.pop());
                    let key: &ValueData = &key.borrow();
                    let object = catch!(self.pop());
                    let property = object
                        .borrow()
                        .get(key)
                        .unwrap_or(Property::new("", new_ref(ValueData::Undefined)));
                    let val = property.value.clone();
                    self.stack.push(val);
                }
                Return => {
                    let return_ = match self.stack.pop() {
                        Some(val) => val,
                        None => new_ref(ValueData::Undefined),
                    };
                    if self.exec_stack.borrow().is_empty() {
                        return;
                    }
                    self.restore_state(true, true, true, true);

                    match self.funs.last() {
                        Some(fun) => {
                            let fun: &mut Function = &mut fun.borrow_mut();
                            match fun {
                                Function::Regular { yield_pos, .. } => {
                                    *yield_pos = None;
                                }
                                _ => (), // do nothing,external function
                            }
                        }
                        None => (), // do nothing
                    }
                    self.funs.pop();

                    self.push_ref(return_);
                }
                Yield => {
                    let return_ = catch!(self.pop());

                    match self.funs.last() {
                        Some(fun) => {
                            let fun: &mut Function = &mut fun.borrow_mut();
                            match fun {
                                Function::Regular {
                                    yield_pos,
                                    yield_env,
                                    ..
                                } => {
                                    match yield_pos {
                                        Some(ref mut pos) => *pos = self.pc,
                                        None => *yield_pos = Some(self.pc),
                                    };
                                    *yield_env = self.env.clone();
                                }
                                _ => unreachable!(),
                            }
                        }
                        None => throw!("can not find function state"),
                    }
                    self.restore_state(true, true, true, true);

                    self.push_ref(return_);
                }
                PushCatch(addr) => {
                    self.exception_stack.push(addr);
                }

                Throw => {
                    let error = match self.stack.pop() {
                        Some(val) => val,
                        None => new_ref(ValueData::Undefined), // TODO: probably be better to throw stack empty exception?
                    };
                    if let Some(location) = self.exception_stack.pop() {
                        self.pc = location;
                        self.push_ref(error);
                        continue;
                    } else {
                        eprintln!("{}", error.borrow());
                        std::process::exit(1);
                    }
                }
                Apply => {
                    let function: Value = catch!(self.pop());
                    let args: Value = catch!(self.pop());
                    let args: &ValueData = &args.borrow();
                    let args = match args {
                        ValueData::Array(array) => array.borrow().clone(),
                        _ => throw!("Array expected in apply"),
                    };
                    let maybe_function = function.borrow();
                    let maybe_function: &ValueData = &maybe_function;
                    match maybe_function {
                        ValueData::Function(fun_) => {
                            let fun_2 = fun_.clone();
                            let fun: &Function = &fun_.borrow();
                            match fun {
                                Function::Native(addr) => {
                                    let fun: fn(
                                        &mut Self,
                                        Value,
                                        &[Value],
                                    )
                                        -> Result<Value, ValueData> =
                                        unsafe { std::mem::transmute(*addr) };

                                    let result = catch!(fun(self, nil(), &args));
                                    self.push_ref(result);
                                }
                                Function::Regular {
                                    environment,
                                    addr,
                                    yield_pos,
                                    code,
                                    args: args_,
                                    yield_env,
                                    constants,
                                    ..
                                } => {
                                    self.funs.push(fun_2);
                                    match yield_pos {
                                        Some(ref pos) => {
                                            self.save_state(true, true, true, true);
                                            self.pc = *pos;

                                            self.env = yield_env.clone();
                                        }
                                        None => {
                                            self.save_state(true, true, true, true);
                                            self.pc = *addr;
                                            self.env = environment.clone();
                                        }
                                    }
                                    self.code = code.clone();
                                    self.m.constants = constants.clone();
                                    for (i, arg) in args_.iter().enumerate() {
                                        if var_declared(&environment, arg) {
                                            catch!(set_variable_in_scope(
                                                &environment,
                                                arg,
                                                args.get(i)
                                                    .unwrap_or(&new_ref(ValueData::Undefined))
                                                    .clone(),
                                                &Position::new(0, 0)
                                            ));
                                        } else {
                                            catch!(declare_var(
                                                &environment,
                                                arg,
                                                args.get(i)
                                                    .unwrap_or(&new_ref(ValueData::Undefined))
                                                    .clone(),
                                                &Position::new(0, 0)
                                            ))
                                        }
                                    }
                                    if var_declared(&environment, "_args") {
                                        catch!(set_variable_in_scope(
                                            &environment,
                                            "_args",
                                            new_ref(ValueData::Array(new_ref(args))),
                                            &Position::new(0, 0)
                                        ))
                                    } else {
                                        catch!(declare_var(
                                            &environment,
                                            "_args",
                                            new_ref(ValueData::Array(new_ref(args))),
                                            &Position::new(0, 0)
                                        ))
                                    }
                                    if var_declared(&environment, "this") {
                                        catch!(set_variable_in_scope(
                                            &environment,
                                            "this",
                                            new_ref(ValueData::Object(new_object())),
                                            &Position::new(0, 0)
                                        ));
                                    } else {
                                        catch!(declare_var(
                                            &environment,
                                            "this",
                                            new_ref(ValueData::Object(new_object())),
                                            &Position::new(0, 0)
                                        ));
                                    }
                                }
                            }
                        }
                        _ => throw!("function expected"),
                    }
                }
                Call(argc) => {
                    let mut args = vec![];
                    let function: Value = catch!(self.pop());
                    let this = catch!(self.pop());

                    for _ in 0..argc {
                        args.push(catch!(self.pop()));
                    }

                    let maybe_function = function.borrow();
                    let maybe_function: &ValueData = &maybe_function;
                    match maybe_function {
                        ValueData::Function(fun_) => {
                            let fun_2 = fun_.clone();
                            let fun: &Function = &fun_.borrow();
                            match fun {
                                Function::Native(addr) => {
                                    let fun: fn(
                                        &mut Self,
                                        Value,
                                        &[Value],
                                    )
                                        -> Result<Value, ValueData> =
                                        unsafe { std::mem::transmute(*addr) };

                                    let result = catch!(fun(self, this, &args));
                                    self.push_ref(result);
                                }
                                Function::Regular {
                                    environment,
                                    addr,
                                    yield_pos,
                                    code,
                                    args: args_,
                                    yield_env,
                                    constants,
                                    ..
                                } => {
                                    self.funs.push(fun_2);
                                    match yield_pos {
                                        Some(ref pos) => {
                                            self.save_state(true, true, true, true);
                                            self.pc = *pos;

                                            self.env = yield_env.clone();
                                        }
                                        None => {
                                            self.save_state(true, true, true, true);
                                            self.pc = *addr;
                                            self.env = environment.clone();
                                        }
                                    }
                                    self.code = code.clone();
                                    self.m.constants = constants.clone();
                                    for (i, arg) in args_.iter().enumerate() {
                                        if var_declared(&environment, arg) {
                                            catch!(set_variable_in_scope(
                                                &environment,
                                                arg,
                                                args.get(i)
                                                    .unwrap_or(&new_ref(ValueData::Undefined))
                                                    .clone(),
                                                &Position::new(0, 0)
                                            ));
                                        } else {
                                            catch!(declare_var(
                                                &environment,
                                                arg,
                                                args.get(i)
                                                    .unwrap_or(&new_ref(ValueData::Undefined))
                                                    .clone(),
                                                &Position::new(0, 0)
                                            ))
                                        }
                                    }
                                    if var_declared(&environment, "_args") {
                                        catch!(set_variable_in_scope(
                                            &environment,
                                            "_args",
                                            new_ref(ValueData::Array(new_ref(args))),
                                            &Position::new(0, 0)
                                        ))
                                    } else {
                                        catch!(declare_var(
                                            &environment,
                                            "_args",
                                            new_ref(ValueData::Array(new_ref(args))),
                                            &Position::new(0, 0)
                                        ))
                                    }
                                    if var_declared(&environment, "this") {
                                        catch!(set_variable_in_scope(
                                            &environment,
                                            "this",
                                            this,
                                            &Position::new(0, 0)
                                        ));
                                    } else {
                                        catch!(declare_var(
                                            &environment,
                                            "this",
                                            this,
                                            &Position::new(0, 0)
                                        ));
                                    }
                                }
                            }
                        }
                        _ => throw!("function expected"),
                    }
                }
                PopCatch => {
                    self.exception_stack.pop();
                }
                Jump(to) => {
                    self.pc = to as usize;
                }
                JumpIf(to) => {
                    let val = catch!(self.pop());
                    let val = val.borrow().clone();
                    if bool::from(val) {
                        self.pc = to as usize;
                    }
                }
                JumpIfFalse(to) => {
                    let val = catch!(self.pop());
                    let val = val.borrow().clone();
                    if !bool::from(val) {
                        self.pc = to as usize;
                    }
                }

                RefEq => {
                    let x: Value = catch!(self.pop());
                    let y: Value = catch!(self.pop());
                    let result = std::sync::Arc::ptr_eq(&x.0, &y.0);
                    let value = ValueData::from(result);
                    self.push(value);
                }
                RefNeq => {
                    let x: Value = catch!(self.pop());
                    let y: Value = catch!(self.pop());
                    let result = !std::sync::Arc::ptr_eq(&x.0, &y.0);
                    let value = ValueData::from(result);
                    self.push(value);
                }

                InitEnv => {
                    let fun = catch!(self.pop());
                    let fun: &ValueData = &fun.borrow();

                    match fun {
                        ValueData::Function(fun) => {
                            let fun: &mut Function = &mut fun.borrow_mut();

                            match fun {
                                Function::Native(_) => {} // TODO: maybe we should throw exception there
                                Function::Regular {
                                    environment,
                                    constants: c,
                                    ..
                                } => {
                                    let env = new_object();
                                    *c = self.m.constants.clone();
                                    set_obj_proto(env.clone(), self.env.clone());
                                    *environment = env;
                                }
                            }
                        }
                        _ => throw!("function expected"),
                    }
                    self.push(fun.clone());
                }
                PushEnv => self.push_env(),
                PopEnv => self.pop_env(),
                Label => (), // nothing to do,relax :D
                Add | Sub | Div | Mul | Rem | Shl | Shr | BitAnd | BitOr | BitXor | And | Or
                | Gt | Ge | Lt | Le | Eq | Ne => {
                    let lhs = catch!(self.pop());
                    let rhs = catch!(self.pop());
                    let lhs = lhs.borrow().clone();
                    let rhs = rhs.borrow().clone();
                    let result: ValueData = match opcode {
                        Add => lhs + rhs,
                        Sub => lhs - rhs,
                        Div => lhs / rhs,
                        Mul => lhs * rhs,
                        Rem => lhs % rhs,
                        Shl => lhs << rhs,
                        Shr => lhs >> rhs,
                        BitAnd => lhs & rhs,
                        BitOr => lhs | rhs,
                        BitXor => lhs ^ rhs,
                        And => ValueData::from(bool::from(lhs) && bool::from(rhs)),
                        Or => ValueData::from(bool::from(lhs) || bool::from(rhs)),
                        Gt => (lhs > rhs).into(),
                        Lt => (lhs < rhs).into(),
                        Le => (lhs <= rhs).into(),
                        Ge => (lhs >= rhs).into(),
                        Eq => (lhs == rhs).into(),
                        Ne => (lhs != rhs).into(),
                        _ => unreachable!(),
                    };
                    self.push(result);
                }
                Opcode::Not => {
                    let val = catch!(self.pop());
                    let val: &ValueData = &val.borrow();
                    let result = match val {
                        ValueData::Bool(boolean) => ValueData::Bool(!*boolean),
                        ValueData::Number(x) => ValueData::Number((!(x.floor() as i64)) as f64),
                        ValueData::Undefined | ValueData::Nil => ValueData::Bool(true),
                        _ => ValueData::Bool(false),
                    };
                    self.push(result);
                }
                Neg => {
                    let val = catch!(self.pop());
                    let val: &ValueData = &val.borrow();
                    let result = match val {
                        ValueData::Number(x) => -*x,
                        ValueData::Nil => 0.0,
                        _ => std::f64::NAN,
                    };
                    self.push(result);
                }

                _ => (),
            }
        }
    }
}
