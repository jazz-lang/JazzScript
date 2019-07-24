use crate::gc::gc;
use crate::gc::*;

pub fn new_ref<T: 'static + Mark>(val: T) -> Ref<T> {
    gc::new_gc(val)
}
pub type Ref<T> = Gc<T>;

use hashlink::LinkedHashMap;

#[derive(Clone)]
pub enum ValueData {
    Nil,

    Undefined,
    Bool(bool),
    Number(f64),
    String(String),
    Object(Ref<Object>),
    Array(Ref<Vec<Value>>),
    Function(Ref<Function>),
}

impl Mark for Object {
    fn mark(&self, gc: &mut InGcEnv) {
        match &self.proto {
            Some(ref proto) => proto.mark_grey(gc),
            None => (),
        }
        for (key, val) in self.table.iter() {
            key.mark(gc);
            val.mark_grey(gc);
        }
    }
}

impl Mark for Function {
    fn mark(&self, gc: &mut InGcEnv) {
        match self {
            Function::Regular { environment, .. } => {
                environment.mark_grey(gc);
            }
            _ => (),
        }
    }
}

impl Mark for ValueData {
    fn mark(&self, gc: &mut InGcEnv) {
        match self {
            ValueData::Object(object) => {
                object.mark_grey(gc);
            }
            ValueData::Array(array) => {
                array.mark_grey(gc);
            }
            ValueData::Function(f) => {
                f.mark_grey(gc);
            }
            _ => (),
        }
    }
}

impl From<ValueData> for i64 {
    fn from(val: ValueData) -> i64 {
        match val {
            ValueData::Number(x) => x as i64,
            ValueData::Nil => 0,
            ValueData::Undefined => 0,
            _ => std::i64::MAX,
        }
    }
}

impl From<ValueData> for f64 {
    fn from(val: ValueData) -> f64 {
        match val {
            ValueData::Number(x) => x,
            ValueData::Nil => 0.0,
            ValueData::Undefined => std::f64::NAN,
            _ => std::f64::NAN,
        }
    }
}

impl From<ValueData> for bool {
    fn from(val: ValueData) -> bool {
        match val {
            ValueData::Number(x) => {
                if x.floor() == 0.0 {
                    false
                } else {
                    true
                }
            }
            ValueData::Bool(x) => x,
            ValueData::Nil => false,
            _ => false,
        }
    }
}

impl From<bool> for ValueData {
    fn from(val: bool) -> ValueData {
        ValueData::Bool(val)
    }
}

impl From<ValueData> for String {
    fn from(val: ValueData) -> String {
        match val {
            ValueData::String(s) => s.clone(),
            ValueData::Number(x) => x.to_string(),
            ValueData::Nil | ValueData::Undefined => String::new(),
            ValueData::Array(_) => format!("{}", val),
            ValueData::Object(_) => format!("{}", val),
            ValueData::Bool(b) => format!("{}", b),
            ValueData::Function(_) => "<function>".to_owned(),
        }
    }
}



#[derive(Clone)]
pub enum Function {
    Native(usize),
    Regular {
        environment: Environment,
        code: Gc<Vec<super::opcodes::Opcode>>, // code of function module,not of function itself
        addr: usize,
        yield_pos: Option<usize>,
        args: Vec<String>,
    },
}

pub trait SetGet {
    fn set(&mut self, _: impl Into<ValueData>, _: impl Into<ValueData>) {
        unimplemented!()
    }
    fn get(&self, _: &ValueData) -> Value {
        unimplemented!()
    }
}

impl SetGet for ValueData {
    fn set(&mut self, key: impl Into<ValueData>, val: impl Into<ValueData>) {
        let key = key.into();
        let val = val.into();
        match self {
            ValueData::Function(func) => {
                let func: &mut Function = &mut func.borrow_mut();
                match func {
                    Function::Regular { environment, .. } => {
                        environment.borrow_mut().set(key, val);
                    }
                    _ => (),
                }
            }
            ValueData::Object(object) => object.borrow_mut().set(key, val),
            ValueData::Array(array) => {
                let mut array = array.borrow_mut();
                let idx = i64::from(key);
                assert!(idx >= 0);
                array[idx as usize] = new_ref(val);
            }
            _ => (),
        }
    }

    fn get(&self, key: &ValueData) -> Value {
        match self {
            ValueData::Function(func) => {
                let func: &Function = &func.borrow();
                match func {
                    Function::Regular { environment, .. } => return environment.borrow().get(key),
                    _ => return new_ref(ValueData::Undefined),
                }
            }
            ValueData::Object(object) => object.borrow().get(key),
            ValueData::Array(array) => {
                let array = array.borrow();
                match key {
                    ValueData::String(s) => {
                        let s: &str = s;
                        match s {
                            "length" => return new_ref(ValueData::Number(array.len() as f64)),
                            _ => (),
                        }
                    }

                    _ => (),
                }

                let idx = i64::from(key.clone());
                assert!(idx >= 0);
                return array[idx as usize].clone();
            }
            _ => new_ref(ValueData::Undefined),
        }
    }
}

use std::fmt;
impl fmt::Display for ValueData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueData::Bool(x) => write!(f, "{}", x),
            ValueData::Number(x) => write!(f, "{}", x),
            ValueData::Function(_) => write!(f, "<function>"),
            ValueData::Nil => write!(f, "nil"),
            ValueData::Undefined => write!(f, "undefined"),
            ValueData::String(s) => write!(f, "{}", s),
            ValueData::Object(object) => {
                let object: &Object = &object.borrow();
                write!(f, "{{")?;
                for (i, (key, val)) in object.table.iter().enumerate() {
                    write!(f, "{}: {}", key, val.borrow())?;
                    if i != object.table.len() - 1 {
                        write!(f, ",")?;
                    }
                }
                write!(f, "}}")
            }
            ValueData::Array(array) => {
                let array = array.borrow();
                write!(f, "[")?;
                for (i, val) in array.iter().enumerate() {
                    write!(f, "{}", val.borrow())?;
                    if i != array.len() - 1 {
                        write!(f, ",")?;
                    }
                }
                write!(f, "]")
            }
        }
    }
}

impl fmt::Debug for ValueData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}
impl PartialEq for ValueData {
    fn eq(&self, other: &Self) -> bool {
        use ValueData::*;
        match (self, other) {
            (Number(x), Number(y)) => x == y,
            (Nil, Nil) => true,
            (Undefined, Undefined) => true,
            (String(x), String(y)) => x == y,
            (Object(x), Object(y)) => {
                let x_ref = x.borrow();
                let y_ref = y.borrow();
                *x_ref == *y_ref
            }
            (Array(x), Array(y)) => *x.borrow() == *y.borrow(),
            (Bool(x), Bool(y)) => x == y,

            _ => false,
        }
    }
}

use std::cmp::Ordering;

impl PartialOrd for ValueData {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => x.partial_cmp(y),
            (ValueData::Array(x), ValueData::Array(y)) => x.borrow().partial_cmp(&y.borrow()),
            (ValueData::Object(obj), ValueData::Object(obj1)) => {
                obj.borrow().partial_cmp(&obj1.borrow())
            }
            (ValueData::String(x), ValueData::String(y)) => x.partial_cmp(y),
            (ValueData::Bool(x), ValueData::Bool(y)) => x.partial_cmp(y),
            _ => None,
        }
    }
}

impl Ord for ValueData {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Eq for ValueData {}

use std::hash::{Hash, Hasher};

impl Hash for ValueData {
    fn hash<H: Hasher>(&self, h: &mut H) {
        match self {
            ValueData::Number(x) => x.to_bits().hash(h),
            ValueData::Nil => 0.hash(h),
            ValueData::Undefined => 0.hash(h),
            ValueData::String(s) => s.hash(h),
            ValueData::Array(array) => {
                let array = array.borrow();
                for x in array.iter() {
                    x.borrow().hash(h);
                }
                array.len().hash(h);
            }
            ValueData::Bool(x) => x.hash(h),
            ValueData::Object(object) => object.borrow().hash(h),
            _ => (-1).hash(h),
        }
    }
}

pub type Value = Ref<ValueData>;

#[derive(Clone)]
pub struct Object {
    pub proto: Option<Ref<Object>>,
    pub table: LinkedHashMap<ValueData, Ref<ValueData>>,
}

use crate::token::Position;
pub fn set_variable_in_scope(
    scopes: &Ref<Object>,
    key: impl Into<ValueData>,
    val: Ref<ValueData>,
    pos: &Position,
) -> Result<(), ValueData> {
    let scopes: &mut Object = &mut scopes.borrow_mut();
    let key = key.into();
    if scopes.table.contains_key(&key) {
        scopes.table.insert(key, val);
        return Ok(());
    }
    if scopes.proto.is_some() {
        return set_variable_in_scope(scopes.proto.as_ref().unwrap(), key, val, pos);
    }
    Err(new_error(
        pos.line as i32,
        None,
        &format!("Variable '{}' not declared", key),
    ))
}

pub fn declare_var(
    scope: &Ref<Object>,
    key: impl Into<ValueData>,
    val: Ref<ValueData>,
    pos: &Position,
) -> Result<(), ValueData> {
    let scope: &mut Object = &mut scope.borrow_mut();
    let key = key.into();
    if scope.table.contains_key(&key) {
        return Err(new_error(
            pos.line as _,
            None,
            &format!("Variable '{}' already declared", key),
        ));
    }
    scope.table.insert(key, val);
    Ok(())
}

pub fn var_declared(scope: &Ref<Object>, key: impl Into<ValueData>) -> bool {
    let scope: &Object = &scope.borrow();
    let key = key.into();
    scope.table.contains_key(&key)
}

pub fn get_variable(
    scope: &Ref<Object>,
    key: impl Into<ValueData>,
    pos: &Position,
) -> Result<Value, ValueData> {
    let scopes: &mut Object = &mut scope.borrow_mut();
    let key = key.into();
    if scopes.table.contains_key(&key) {
        return Ok(scopes.table.get(&key).unwrap().clone());
    }
    if scopes.proto.is_some() {
        return get_variable(scopes.proto.as_ref().unwrap(), key, pos);
    }
    Err(new_error(
        pos.line as i32,
        None,
        &format!("Variable '{}' not declared", key),
    ))
}

impl SetGet for Object {
    fn set(&mut self, key: impl Into<ValueData>, val: impl Into<ValueData>) {
        self.table.insert(key.into(), new_ref(val.into()));
    }
    fn get(&self, key: &ValueData) -> Value {
        match key {
            ValueData::String(name) => {
                let name: &str = name;
                match name {
                    "__proto__" => {
                        return match &self.proto {
                            Some(proto) => new_ref(ValueData::Object(proto.clone())),
                            None => new_ref(ValueData::Undefined),
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        };

        self.table
            .get(key)
            .unwrap_or(&new_ref(ValueData::Undefined))
            .clone()
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.table == other.table
            && match (&self.proto, &other.proto) {
                (Some(x), Some(y)) => *x.borrow() == *y.borrow(),
                (None, None) => true,
                _ => false,
            }
    }
}

impl PartialOrd for Object {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.table.partial_cmp(&other.table)
    }
}

impl Eq for Object {}

impl Hash for Object {
    fn hash<H: Hasher>(&self, h: &mut H) {
        for (key, val) in self.table.iter() {
            key.hash(h);
            val.borrow().hash(h);
        }
        self.table.len().hash(h);
        match &self.proto {
            Some(proto) => proto.borrow().hash(h),
            None => (),
        }
    }
}

pub type Environment = Ref<Object>;

pub fn new_object() -> Ref<Object> {
    new_ref(Object {
        proto: None,
        table: LinkedHashMap::new(),
    })
}

impl Into<ValueData> for String {
    fn into(self) -> ValueData {
        ValueData::String(self.to_owned())
    }
}

impl Into<ValueData> for &str {
    fn into(self) -> ValueData {
        ValueData::String(self.to_owned())
    }
}

impl Into<ValueData> for &String {
    fn into(self) -> ValueData {
        ValueData::String(self.to_owned())
    }
}
macro_rules! into_num {
    ($($t: ty)*) => {
        $(
        impl From<$t> for ValueData {
            fn from(x: $t) -> ValueData {
                ValueData::Number(x as f64)
            }
        }

        )*
    };
}

into_num!(
    f32 f64
    i8 i16 i32
    i64 i128
    u8 u32 u64 usize u16 u128
);

impl<T: Into<ValueData>> From<Option<T>> for ValueData {
    fn from(val: Option<T>) -> ValueData {
        match val {
            Some(x) => x.into(),
            None => ValueData::Nil,
        }
    }
}

pub fn new_error(line: i32, file: Option<&str>, err: &str) -> ValueData {
    let object = new_object();
    let proto = new_object();
    proto.borrow_mut().set("__name__", "JLRuntimeError");
    object.borrow_mut().proto = Some(proto);
    object.borrow_mut().set("line", line);
    object.borrow_mut().set("file", file);
    object.borrow_mut().set("error", err);

    ValueData::Object(object)
}

pub fn instanceof(obj: &Ref<Object>, of: &Ref<Object>) -> bool {
    let of = of.borrow();
    if obj.borrow().proto.is_none() {
        return false;
    }

    *obj.borrow().proto.as_ref().unwrap().borrow() == *of
}

use std::ops::*;

impl Add for ValueData {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => ValueData::Number(x + y),
            (ValueData::Array(x), ValueData::Array(y)) => {
                let mut array = vec![];
                for x in x.borrow().iter() {
                    array.push(x.clone());
                }

                for y in y.borrow().iter() {
                    array.push(y.clone());
                }

                return ValueData::Array(new_ref(array));
            }
            (ValueData::String(x), val) => ValueData::String(format!("{}{}", x, val)),
            (val, ValueData::String(x)) => ValueData::String(format!("{}{}", val, x)),
            _ => ValueData::Undefined,
        }
    }
}

impl Sub for ValueData {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => ValueData::Number(x - y),

            _ => ValueData::Undefined,
        }
    }
}

impl Mul for ValueData {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => ValueData::Number(x * y),

            _ => ValueData::Undefined,
        }
    }
}
impl Div for ValueData {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => ValueData::Number(x / y),

            _ => ValueData::Undefined,
        }
    }
}

impl Rem for ValueData {
    type Output = Self;
    fn rem(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => ValueData::Number(x % y),

            _ => ValueData::Undefined,
        }
    }
}

impl Shr for ValueData {
    type Output = Self;
    fn shr(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => {
                ValueData::Number(((x.floor() as i64) >> y.floor() as i64) as f64)
            }

            _ => ValueData::Undefined,
        }
    }
}

impl Shl for ValueData {
    type Output = Self;
    fn shl(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => {
                ValueData::Number(((x.floor() as i64) << y.floor() as i64) as f64)
            }

            _ => ValueData::Undefined,
        }
    }
}

impl BitXor for ValueData {
    type Output = Self;
    fn bitxor(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => {
                ValueData::Number(((x.floor() as i64) ^ y.floor() as i64) as f64)
            }

            _ => ValueData::Undefined,
        }
    }
}

impl BitAnd for ValueData {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => {
                ValueData::Number(((x.floor() as i64) & y.floor() as i64) as f64)
            }

            _ => ValueData::Undefined,
        }
    }
}

impl BitOr for ValueData {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        match (self, other) {
            (ValueData::Number(x), ValueData::Number(y)) => {
                ValueData::Number(((x.floor() as i64) | y.floor() as i64) as f64)
            }

            _ => ValueData::Undefined,
        }
    }
}