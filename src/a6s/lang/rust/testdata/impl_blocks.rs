// Test fixture: Impl blocks

struct Point(i32, i32);

impl Point {
    fn method(&self) {}
}

trait Shape {
    fn area(&self) -> f64;
}

impl Shape for Point {
    fn area(&self) -> f64 {
        0.0
    }
}

struct Generic<T>(T);

impl<T: Clone> Generic<T> {
    fn get(&self) -> T {
        self.0.clone()
    }
}

impl<T> Clone for Generic<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Generic(self.0.clone())
    }
}
