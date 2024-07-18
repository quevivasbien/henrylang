use criterion::{criterion_group, criterion_main, Criterion};

use henrylang::*;

macro_rules! run_expect_value {
    ($source:expr, $variant:ident) => {
        match VM::new().interpret($source)
            .unwrap()
        {
            values::TaggedValue::$variant(x) => x,
            _ => panic!("Should be a {}", stringify!($variant)),
        }
    }
}

fn approx_e() -> f64 {
    let source = "
    factorial := |x: Int| {
        if x <= 1 {
            1
        }
        else {
            prod(1 to x)
        }
    }
    
    approx_e := |n: Int|: Float {
        if n = 0 {
            1.0
        }
        else {
            1.0 / float(factorial(n)) + approx_e(n-1)
        }
    }
    
    approx_e(20)
    ";
    run_expect_value!(source, Float)
}

fn arcsin() -> f64 {
    let source = "
    asin := |x: Float, n: Int| {
        iter := |n: Int|: Float {
            if n = 0 {
                x
            }
            else {
                prod(|x:Int|{float(x)/float(x+1)} -> filter(|x:Int|{mod(x, 2)=1}, 1 to (2*n)))
                    * pow(x, float(n*2+1)) / float(n*2+1)
                    + iter(n-1)
            }
        }
        iter(n)
    }
    
    asin(0.5, 100)
    ";
    run_expect_value!(source, Float)
}

fn fibonacci() -> i64 {
    let source = "
    fib_helper := |n: Int, x: Int, y: Int|: Int {
        z := x + y
        if n = 1 {
            z
        }
        else {
            fib_helper(n - 1, y, z)
        }
    }
    
    fib := |n: Int| {
        if n < 3 {
            1
        }
        else {
            fib_helper(n - 2, 1, 1)
        }
    }
    
    fib(80)
    ";

    run_expect_value!(source, Int)
}

fn sum_primes() -> i64 {
    let source = "
    is_prime := |n: Int| {
        if n = 2 { true }
        else {
            sqrt_n := int(sqrt(float(n))) + 1
            all(|p: Int| { mod(n, p) != 0 } -> 2 to sqrt_n)
        }
    }
    
    primes := filter(is_prime, 2 to 10000)
    sum(primes)
    ";
    run_expect_value!(source, Int)
}

fn criterion_taylor_series(c: &mut Criterion) {
    let mut group = c.benchmark_group("taylor_series");
    group.sample_size(50);
    group.bench_function(
        "euler",
        move |b| b.iter(approx_e)
    );
    group.bench_function(
        "arcsin",
        move |b| b.iter(arcsin)
    );
    group.finish();
}

fn criterion_fibonacci(c: &mut Criterion) {
    c.bench_function(
        "fibonacci",
        move |b| b.iter(fibonacci)
    );
}

fn criterion_primes(c: &mut Criterion) {
    let mut group = c.benchmark_group("primes");
    group.sample_size(80);
    group.bench_function(
        "sum_primes",
        move |b| b.iter(sum_primes)
    );
    group.finish();
}

criterion_group!(benches, criterion_taylor_series, criterion_fibonacci, criterion_primes);
criterion_main!(benches);
