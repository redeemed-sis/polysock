pub mod oneliner;

pub trait Command {
    fn execute(&mut self);
}
