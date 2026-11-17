//! The multilayer perceptron of MLP: the network, error backpropagation, the optimizers and
//! the stopping rules, weight initialization by alternating simulated annealing with training,
//! and the automatic choice of the number of hidden units.
//!
//! Rules, in our words, from the Algorithms manual (MLP):
//! - Layer i has J_i units and a bias; unit k of layer i gets c = sum_j w(i;j,k) a(i-1;j) with
//!   a(i-1;0) = 1, and gives gamma_i(c). Hidden layers use tanh or the sigmoid; the output
//!   layer the identity, sigmoid, tanh or softmax (softmax within each categorical dependent).
//! - The error of a set of cases is the sum over them of half the squared differences, or with
//!   softmax the cross-entropy -sum y ln a over the categories.
//! - Backpropagation: at the output delta = a - y for cross-entropy, gamma'(c)(a - y)
//!   otherwise; a weight's derivative is delta times the unit it comes from; a hidden unit's
//!   delta is gamma'(c) times the sum of the deltas above it through its weights.
//! - Batch gradient descent: v = w - eta g + alpha dw, with alpha cut to 0.9 eta |g| / |dw|
//!   when eta |g| < alpha |dw|; v is taken when it lowers the error, else eta is halved and
//!   the step tried again. Stops at |g| < 1e-6.
//! - Online and mini-batch: the same step on each group of cases, kept only when it lowers
//!   the group's error; eta shrinks by exp(-beta) per group to no lower than eta_low, with
//!   beta = ln(eta0 / eta_low) / (p K), K groups per pass and p LEARNINGEPOCHS.
//! - Scaled conjugate gradient (batch only), from lambda0 and sigma0, restarting the
//!   direction every N successful steps (N weights), finishing at |r| < 1e-6.
//! - Stopping rules after each step (an iteration in batch, a data pass otherwise), in order:
//!   no new minimum of the monitored error for n steps; the time limit; the maximum number of
//!   epochs (max(2N + 1, 100) by default); the relative change of the training error at a new
//!   minimum below ERRORCHANGE; the training error over that of the mean model below
//!   ERRORRATIO. The weights kept are those of the step with the lowest monitored error.
//! - Initial weights: alternated simulated annealing on a sample of at most min(1000,
//!   MEMSIZE) cases: the best of 4 random vectors in [a0 - a, a0 + a], trained; while the error
//!   ratio exceeds 0.05, up to 4 times, 4 perturbations of size 0.5^k a and the best of them
//!   trained when it improves; the whole repeated up to 3 times while the ratio exceeds 0.1.

use statscraft_syntax::{Activation, Training};

/// A network's shape; the weights are kept apart, as one vector.
#[derive(Clone, Debug)]
pub(crate) struct Net {
    /// Units per layer, input first, without the bias units.
    pub sizes: Vec<usize>,
    pub hidden: Activation,
    pub output: Activation,
    /// The output units of each categorical dependent, for softmax.
    pub groups: Vec<std::ops::Range<usize>>,
}

fn activate(f: Activation, c: &mut [f64], groups: &[std::ops::Range<usize>]) {
    match f {
        Activation::Tanh => c.iter_mut().for_each(|v| *v = v.tanh()),
        Activation::Sigmoid => c.iter_mut().for_each(|v| *v = 1.0 / (1.0 + (-*v).exp())),
        Activation::Identity => {}
        Activation::Softmax => {
            for g in groups {
                let s = &mut c[g.clone()];
                let top = s.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                s.iter_mut().for_each(|v| *v = (*v - top).exp());
                let sum: f64 = s.iter().sum();
                s.iter_mut().for_each(|v| *v /= sum);
            }
        }
    }
}

/// The derivative of the activation at the unit's output a.
fn slope(f: Activation, a: f64) -> f64 {
    match f {
        Activation::Tanh => 1.0 - a * a,
        Activation::Sigmoid => a * (1.0 - a),
        _ => 1.0,
    }
}

/// A case: the input units and the target units.
pub(crate) struct Row {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

impl Net {
    pub fn weights(&self) -> usize {
        self.sizes.windows(2).map(|w| (w[0] + 1) * w[1]).sum()
    }

    fn function(&self, layer: usize) -> Activation {
        match layer + 1 == self.sizes.len() {
            true => self.output,
            false => self.hidden,
        }
    }

    pub fn cross_entropy(&self) -> bool {
        self.output == Activation::Softmax
    }

    /// The units of every layer, input first.
    pub fn forward(&self, w: &[f64], x: &[f64]) -> Vec<Vec<f64>> {
        let mut layers = vec![x.to_vec()];
        let mut at = 0;
        for i in 1..self.sizes.len() {
            let below = &layers[i - 1];
            let n = below.len() + 1;
            let mut c: Vec<f64> = (0..self.sizes[i])
                .map(|k| {
                    let row = &w[at + k * n..at + (k + 1) * n];
                    row[0] + row[1..].iter().zip(below).map(|(a, b)| a * b).sum::<f64>()
                })
                .collect();
            at += n * self.sizes[i];
            activate(self.function(i), &mut c, &self.groups);
            layers.push(c);
        }
        layers
    }

    pub fn predict(&self, w: &[f64], x: &[f64]) -> Vec<f64> {
        self.forward(w, x).pop().expect("an output layer")
    }

    pub fn case_error(&self, a: &[f64], y: &[f64]) -> f64 {
        match self.cross_entropy() {
            true => -a
                .iter()
                .zip(y)
                .filter(|(_, y)| **y > 0.0)
                .map(|(a, y)| y * a.max(1e-300).ln())
                .sum::<f64>(),
            false => 0.5 * a.iter().zip(y).map(|(a, y)| (y - a) * (y - a)).sum::<f64>(),
        }
    }

    pub fn error(&self, w: &[f64], rows: &[Row], set: &[usize]) -> f64 {
        set.iter()
            .map(|&m| self.case_error(&self.predict(w, &rows[m].x), &rows[m].y))
            .sum()
    }

    /// The error and its gradient over the set.
    pub fn gradient(&self, w: &[f64], rows: &[Row], set: &[usize]) -> (f64, Vec<f64>) {
        let mut g = vec![0.0; w.len()];
        let mut e = 0.0;
        let last = self.sizes.len() - 1;
        let offsets: Vec<usize> = (0..=last)
            .scan(0, |at, i| {
                let here = *at;
                if i > 0 {
                    *at += (self.sizes[i - 1] + 1) * self.sizes[i];
                }
                Some(here)
            })
            .collect();
        for &m in set {
            let a = self.forward(w, &rows[m].x);
            let y = &rows[m].y;
            e += self.case_error(&a[last], y);
            let mut delta: Vec<f64> = a[last]
                .iter()
                .zip(y)
                .map(|(a, y)| match self.cross_entropy() {
                    true => a - y,
                    false => slope(self.output, *a) * (a - y),
                })
                .collect();
            for i in (1..=last).rev() {
                let n = self.sizes[i - 1] + 1;
                let at = offsets[i];
                let mut below = vec![0.0; n - 1];
                for (k, d) in delta.iter().enumerate() {
                    let base = at + k * n;
                    g[base] += d;
                    for j in 1..n {
                        g[base + j] += d * a[i - 1][j - 1];
                        below[j - 1] += d * w[base + j];
                    }
                }
                delta = below
                    .iter()
                    .zip(&a[i - 1])
                    .map(|(s, a)| s * slope(self.hidden, *a))
                    .collect();
            }
        }
        (e, g)
    }

    /// The error of the model that predicts every target unit by its mean over the set.
    pub fn null_error(&self, rows: &[Row], set: &[usize]) -> f64 {
        let r = self.sizes[self.sizes.len() - 1];
        let n = set.len().max(1) as f64;
        let mean: Vec<f64> = (0..r)
            .map(|t| set.iter().map(|&m| rows[m].y[t]).sum::<f64>() / n)
            .collect();
        set.iter()
            .map(|&m| self.case_error(&mean, &rows[m].y))
            .sum()
    }
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// How training goes and when it stops.
#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub training: Training,
    pub scaled_conjugate: bool,
    pub eta0: f64,
    pub eta_low: f64,
    pub momentum: f64,
    pub learning_epochs: usize,
    pub lambda0: f64,
    pub sigma0: f64,
    /// Cases per mini-batch (1 for online).
    pub batch: usize,
    pub error_steps: usize,
    pub both: bool,
    pub max_minutes: Option<f64>,
    pub max_epochs: Option<usize>,
    pub error_change: f64,
    pub error_ratio: f64,
}

impl Settings {
    /// Batch training by scaled conjugate gradient with the default rules and no timer.
    pub fn defaults() -> Settings {
        Settings {
            training: Training::Batch,
            scaled_conjugate: true,
            eta0: 0.4,
            eta_low: 0.001,
            momentum: 0.9,
            learning_epochs: 10,
            lambda0: 0.0000005,
            sigma0: 0.00005,
            batch: 1,
            error_steps: 1,
            both: false,
            max_minutes: None,
            max_epochs: None,
            error_change: 0.0001,
            error_ratio: 0.001,
        }
    }
}

/// Why training stopped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Stop {
    ErrorSteps(usize),
    Time,
    Epochs(usize),
    ErrorChange(f64),
    ErrorRatio(f64),
    /// The gradient vanished.
    Converged,
}

pub(crate) struct Trained {
    pub w: Vec<f64>,
    pub stop: Stop,
}

/// The optimizer's state between steps.
enum Optimizer {
    Scg {
        lambda: f64,
        lambda_bar: f64,
        r: Vec<f64>,
        p: Vec<f64>,
        success: bool,
        delta: f64,
        steps: usize,
    },
    Descent {
        eta: f64,
        dw: Vec<f64>,
    },
}

/// One step of the optimizer; false when it cannot go on (the gradient vanished).
#[allow(clippy::too_many_arguments)]
fn step(
    net: &Net,
    s: &Settings,
    w: &mut Vec<f64>,
    opt: &mut Optimizer,
    rows: &[Row],
    train: &[usize],
    order: &[usize],
    cursor: &mut usize,
) -> bool {
    match opt {
        Optimizer::Scg {
            lambda,
            lambda_bar,
            r,
            p,
            success,
            delta,
            steps,
        } => loop {
            let p2 = dot(p, p);
            if *success {
                let sigma = s.sigma0 / p2.sqrt();
                let moved: Vec<f64> = w.iter().zip(p.iter()).map(|(w, p)| w + sigma * p).collect();
                let g1 = net.gradient(&moved, rows, train).1;
                // r is minus the gradient at w
                *delta = p
                    .iter()
                    .zip(g1.iter().zip(r.iter()))
                    .map(|(p, (g1, r))| p * (g1 + r) / sigma)
                    .sum();
            }
            let mut d = *delta + (*lambda - *lambda_bar) * p2;
            if d <= 0.0 {
                *lambda_bar = 2.0 * (*lambda - d / p2);
                d = -d + *lambda * p2;
                *lambda = *lambda_bar;
            }
            let mu = dot(p, r);
            let alpha = mu / d;
            let e0 = net.error(w, rows, train);
            let v: Vec<f64> = w.iter().zip(p.iter()).map(|(w, p)| w + alpha * p).collect();
            let comparison = 2.0 * d * (e0 - net.error(&v, rows, train)) / (mu * mu);
            *success = comparison >= 0.0;
            if *success {
                *w = v;
                let r1: Vec<f64> = net.gradient(w, rows, train).1.iter().map(|g| -g).collect();
                *lambda_bar = 0.0;
                *steps += 1;
                if norm(&r1) < 1e-6 {
                    return false;
                }
                if *steps % w.len() == 0 {
                    *p = r1.clone();
                } else {
                    let beta = (dot(&r1, &r1) - dot(&r1, r)) / mu;
                    *p = r1.iter().zip(p.iter()).map(|(r, p)| r + beta * p).collect();
                }
                *r = r1;
                if comparison >= 0.75 {
                    *lambda /= 4.0;
                }
            } else {
                *lambda_bar = *lambda;
            }
            if comparison < 0.25 {
                *lambda += d * (1.0 - comparison) / p2;
            }
            if *success {
                return true;
            }
            if !lambda.is_finite() || *lambda > 1e100 || !mu.is_finite() {
                return false;
            }
        },
        Optimizer::Descent { eta, dw } if s.training == Training::Batch => {
            let (e, g) = net.gradient(w, rows, train);
            let gn = norm(&g);
            if gn < 1e-6 {
                return false;
            }
            loop {
                let v = descent(w, &g, dw, *eta, s.momentum);
                if net.error(&v, rows, train) < e {
                    *dw = v.iter().zip(w.iter()).map(|(a, b)| a - b).collect();
                    *w = v;
                    return true;
                }
                *eta *= 0.5;
                if *eta < 1e-30 {
                    return false;
                }
            }
        }
        Optimizer::Descent { eta, dw } => {
            let m = order.len();
            let size = s.batch.min(m);
            let groups = m.div_ceil(size);
            let beta = (s.eta0 / s.eta_low).ln() / (s.learning_epochs * groups) as f64;
            for _ in 0..groups {
                let group: Vec<usize> = (0..size).map(|i| order[(*cursor + i) % m]).collect();
                *cursor = (*cursor + size) % m;
                let (e, g) = net.gradient(w, rows, &group);
                let v = descent(w, &g, dw, *eta, s.momentum);
                if net.error(&v, rows, &group) < e {
                    *dw = v.iter().zip(w.iter()).map(|(a, b)| a - b).collect();
                    *w = v;
                }
                *eta = (*eta * (-beta).exp()).max(s.eta_low);
            }
            true
        }
    }
}

/// w - eta g + alpha dw, with alpha cut so that the gradient leads.
fn descent(w: &[f64], g: &[f64], dw: &[f64], eta: f64, momentum: f64) -> Vec<f64> {
    let (gn, dn) = (norm(g), norm(dw));
    let alpha = match eta * gn < momentum * dn {
        true => 0.9 * eta * gn / dn,
        false => momentum,
    };
    (0..w.len())
        .map(|i| w[i] - eta * g[i] + alpha * dw[i])
        .collect()
}

/// Trains from `w` until a stopping rule holds. `order` is the order of the training cases
/// for online and mini-batch training.
pub(crate) fn train(
    net: &Net,
    s: &Settings,
    mut w: Vec<f64>,
    rows: &[Row],
    train: &[usize],
    test: &[usize],
    order: &[usize],
) -> Trained {
    let started = std::time::Instant::now();
    let batch = s.training == Training::Batch;
    let mut opt = match s.scaled_conjugate && batch {
        true => {
            let r: Vec<f64> = net.gradient(&w, rows, train).1.iter().map(|g| -g).collect();
            Optimizer::Scg {
                lambda: s.lambda0,
                lambda_bar: 0.0,
                p: r.clone(),
                r,
                success: true,
                delta: 0.0,
                steps: 0,
            }
        }
        false => Optimizer::Descent {
            eta: s.eta0,
            dw: vec![0.0; w.len()],
        },
    };
    let max_epochs = s.max_epochs.unwrap_or((2 * w.len() + 1).max(100));
    let null = net.null_error(rows, train);
    // the error rules on the training data count unless online training watches testing only
    let training_rules = batch || test.is_empty() || s.both;
    let watch_train = !batch && (test.is_empty() || s.both);
    let mut last = net.error(&w, rows, train);
    let mut best_train = (last, 0, w.clone());
    let mut best_test = (!test.is_empty()).then(|| (net.error(&w, rows, test), 0, w.clone()));
    let mut cursor = 0;
    let mut k = 0;
    let stop = loop {
        let going = step(net, s, &mut w, &mut opt, rows, train, order, &mut cursor);
        k += 1;
        let e = net.error(&w, rows, train);
        if e < best_train.0 {
            best_train = (e, k, w.clone());
        }
        if let Some(best) = best_test.as_mut() {
            let et = net.error(&w, rows, test);
            if et < best.0 {
                *best = (et, k, w.clone());
            }
        }
        if !going {
            break Stop::Converged;
        }
        let stalled = |best: &(f64, usize, Vec<f64>)| k - best.1 >= s.error_steps;
        if best_test.as_ref().is_some_and(stalled) || (watch_train && stalled(&best_train)) {
            break Stop::ErrorSteps(s.error_steps);
        }
        if s.max_minutes
            .is_some_and(|m| started.elapsed().as_secs_f64() > 60.0 * m)
        {
            break Stop::Time;
        }
        if k >= max_epochs {
            break Stop::Epochs(max_epochs);
        }
        if training_rules
            && best_train.1 == k
            && (e - last).abs() / (0.5 * (e + last) + 1e-10) < s.error_change
        {
            break Stop::ErrorChange(s.error_change);
        }
        if training_rules && e / (null + 1e-10) < s.error_ratio {
            break Stop::ErrorRatio(s.error_ratio);
        }
        last = e;
    };
    let w = match best_test {
        Some((_, k2, w2)) if k2 < best_train.1 => w2,
        _ => best_train.2,
    };
    Trained { w, stop }
}

/// Uniform numbers from the session's generator.
pub(crate) type Draw<'a> = &'a mut dyn FnMut() -> f64;

/// The sample that initialization and the automatic architecture work on.
pub(crate) struct Sample {
    pub train: Vec<usize>,
    pub test: Vec<usize>,
}

impl Sample {
    /// At most `size` of the training and testing cases at random; with no testing cases, 30%
    /// of the sample tests.
    pub fn draw(train: &[usize], test: &[usize], size: usize, u: Draw) -> Sample {
        let mut all: Vec<(usize, bool)> = train
            .iter()
            .map(|&m| (m, true))
            .chain(test.iter().map(|&m| (m, false)))
            .collect();
        shuffle(&mut all, u);
        all.truncate(size);
        if test.is_empty() {
            let cut = (0.7 * all.len() as f64).round() as usize;
            // clamp(1, 0) would panic on an empty sample
            let cut = cut.clamp(all.len().min(1), all.len());
            return Sample {
                train: all[..cut].iter().map(|c| c.0).collect(),
                test: all[cut..].iter().map(|c| c.0).collect(),
            };
        }
        Sample {
            train: all.iter().filter(|c| c.1).map(|c| c.0).collect(),
            test: all.iter().filter(|c| !c.1).map(|c| c.0).collect(),
        }
    }
}

/// Fisher-Yates with the session's generator.
pub(crate) fn shuffle<T>(items: &mut [T], u: Draw) {
    for i in (1..items.len()).rev() {
        let j = ((u() * (i + 1) as f64) as usize).min(i);
        items.swap(i, j);
    }
}

/// The ratio of the training error to that of the mean model.
fn ratio(net: &Net, w: &[f64], rows: &[Row], set: &[usize]) -> f64 {
    net.error(w, rows, set) / (net.null_error(rows, set) + 1e-10)
}

/// Alternated simulated annealing and training on the sample; from `start` when given (then
/// only once, without the random start). Returns the weights and their test error.
pub(crate) fn anneal(
    net: &Net,
    rows: &[Row],
    sample: &Sample,
    (center, offset): (f64, f64),
    start: Option<Vec<f64>>,
    u: Draw,
) -> (Vec<f64>, f64) {
    let s = Settings::defaults();
    let n = net.weights();
    let tested = |w: &[f64]| match sample.test.is_empty() {
        true => net.error(w, rows, &sample.train),
        false => net.error(w, rows, &sample.test),
    };
    let best_of = |candidates: Vec<Vec<f64>>| {
        candidates
            .into_iter()
            .map(|w| (net.error(&w, rows, &sample.train), w))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .expect("candidates")
    };
    let repeats = if start.is_some() { 1 } else { 3 };
    let mut start = start;
    let mut result: Option<(Vec<f64>, f64)> = None;
    for _ in 0..repeats {
        let mut w = start.take().unwrap_or_else(|| {
            let random = (0..4)
                .map(|_| {
                    (0..n)
                        .map(|_| center + offset * (2.0 * u() - 1.0))
                        .collect()
                })
                .collect();
            best_of(random).1
        });
        for k1 in 0.. {
            w = train(net, &s, w, rows, &sample.train, &sample.test, &sample.train).w;
            if ratio(net, &w, rows, &sample.train) <= 0.05 || k1 + 1 >= 4 {
                break;
            }
            let size = 0.5f64.powi(k1 + 1) * offset;
            let shaken = (0..4)
                .map(|_| w.iter().map(|x| x + size * (2.0 * u() - 1.0)).collect())
                .collect();
            let (e, candidate) = best_of(shaken);
            if e >= net.error(&w, rows, &sample.train) {
                break;
            }
            w = candidate;
        }
        let e = tested(&w);
        let fit = ratio(net, &w, rows, &sample.train);
        if result.as_ref().is_none_or(|r| e < r.1) {
            result = Some((w, e));
        }
        if fit <= 0.1 {
            break;
        }
    }
    result.expect("one repeat")
}

/// The weights of a one-hidden-layer network without hidden unit `j`.
fn without(net: &Net, w: &[f64], j: usize) -> Vec<f64> {
    let (p, h) = (net.sizes[0] + 1, net.sizes[1]);
    let mut out: Vec<f64> = w[..h * p]
        .chunks(p)
        .enumerate()
        .filter(|(k, _)| *k != j)
        .flat_map(|(_, c)| c.to_vec())
        .collect();
    for c in w[h * p..].chunks(h + 1) {
        out.extend(
            c.iter()
                .enumerate()
                .filter(|(k, _)| *k != j + 1)
                .map(|(_, v)| v),
        );
    }
    out
}

/// The weights with one more hidden unit, its weights drawn at random.
fn with_one_more(net: &Net, w: &[f64], interval: (f64, f64), u: Draw) -> Vec<f64> {
    let (p, h) = (net.sizes[0] + 1, net.sizes[1]);
    let mut random = || interval.0 + interval.1 * (2.0 * u() - 1.0);
    let mut out = w[..h * p].to_vec();
    out.extend((0..p).map(|_| random()));
    for c in w[h * p..].chunks(h + 1) {
        out.extend(c);
        out.push(random());
    }
    out
}

/// The automatic architecture: the number of units in one hidden layer, from `k0` within
/// [min, max], and the weights found for it on the sample.
pub(crate) fn choose_units(
    net: &Net,
    rows: &[Row],
    sample: &Sample,
    (k0, min, max): (usize, usize, usize),
    interval: (f64, f64),
    u: Draw,
) -> (usize, Vec<f64>) {
    let shaped = |k: usize| Net {
        sizes: vec![net.sizes[0], k, net.sizes[2]],
        ..net.clone()
    };
    let train_ratio = |k: usize, w: &[f64]| ratio(&shaped(k), w, rows, &sample.train);
    let mut overall: Option<(usize, Vec<f64>, f64)> = None;
    for _ in 0..5 {
        let (w0, e0) = anneal(&shaped(k0), rows, sample, interval, None, u);
        let mut tried = vec![(k0, w0.clone(), e0)];
        let mut down = k0 > min;
        if !down && train_ratio(k0, &w0) <= 0.01 {
            return (k0, w0);
        }
        let (mut k, mut w, mut e) = (k0, w0.clone(), e0);
        let mut go_on = down || k0 < max;
        while go_on {
            let (k1, start) = match down {
                true => {
                    let weakest = (0..k)
                        .map(|j| {
                            let less = without(&shaped(k), &w, j);
                            let err = match sample.test.is_empty() {
                                true => shaped(k - 1).error(&less, rows, &sample.train),
                                false => shaped(k - 1).error(&less, rows, &sample.test),
                            };
                            (err, less)
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .expect("hidden units");
                    (k - 1, weakest.1)
                }
                false => (k + 1, with_one_more(&shaped(k), &w, interval, u)),
            };
            let (w1, e1) = anneal(&shaped(k1), rows, sample, interval, Some(start), u);
            tried.push((k1, w1.clone(), e1));
            let dropped = e1 < e;
            (k, w, e) = (k1, w1, e1);
            go_on = match (dropped, down) {
                (true, false) => k < max && train_ratio(k, &w) > 0.01,
                (true, true) => k > min,
                (false, true) if k0.abs_diff(k) <= 1 && train_ratio(k0, &w0) > 0.01 => {
                    down = false;
                    (k, w, e) = (k0, w0.clone(), e0);
                    k < max
                }
                _ => false,
            };
        }
        // the lowest test error; on a tie the fewer units
        let best = tried
            .into_iter()
            .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)))
            .expect("tried");
        let fit = train_ratio(best.0, &best.1);
        if overall.as_ref().is_none_or(|o| best.2 < o.2) {
            overall = Some(best);
        }
        if fit <= 0.1 {
            break;
        }
    }
    let (k, w, _) = overall.expect("one round");
    (k, w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_sample_draws_nothing() {
        let mut u = || 0.5;
        let s = Sample::draw(&[], &[], 10, &mut u);
        assert!(s.train.is_empty() && s.test.is_empty());
    }

    fn tiny() -> (Net, Vec<f64>, Vec<Row>) {
        // 1 input, 1 tanh hidden unit, 1 identity output
        let net = Net {
            sizes: vec![1, 1, 1],
            hidden: Activation::Tanh,
            output: Activation::Identity,
            groups: Vec::new(),
        };
        // hidden: bias 0.1, weight 0.5; output: bias -0.2, weight 0.8
        let w = vec![0.1, 0.5, -0.2, 0.8];
        let rows = vec![Row {
            x: vec![2.0],
            y: vec![1.0],
        }];
        (net, w, rows)
    }

    #[test]
    fn forward_pass_and_gradient_of_a_tiny_network_by_hand() {
        let (net, w, rows) = tiny();
        // by hand: c1 = 0.1 + 0.5 * 2 = 1.1, h = tanh(1.1), a = -0.2 + 0.8 h
        let h = 1.1f64.tanh();
        let a = -0.2 + 0.8 * h;
        let layers = net.forward(&w, &[2.0]);
        assert!((layers[1][0] - h).abs() < 1e-15);
        assert!((layers[2][0] - a).abs() < 1e-15);
        let (e, g) = net.gradient(&w, &rows, &[0]);
        assert!((e - 0.5 * (1.0 - a) * (1.0 - a)).abs() < 1e-15);
        // dE/da = a - y; output weights get it times (1, h); the hidden ones times
        // 0.8 (1 - h^2) (1, x)
        let d = a - 1.0;
        let dh = d * 0.8 * (1.0 - h * h);
        let expected = [dh, dh * 2.0, d, d * h];
        for (got, want) in g.iter().zip(expected) {
            assert!((got - want).abs() < 1e-14, "{g:?}");
        }
    }

    #[test]
    fn gradient_agrees_with_finite_differences_for_two_layers_and_softmax() {
        let net = Net {
            sizes: vec![2, 3, 2, 3],
            hidden: Activation::Sigmoid,
            output: Activation::Softmax,
            groups: std::iter::once(0..3).collect(),
        };
        let w: Vec<f64> = (0..net.weights())
            .map(|i| ((i * 7 % 11) as f64 - 5.0) / 10.0)
            .collect();
        let rows = vec![
            Row {
                x: vec![0.3, -1.2],
                y: vec![0.0, 1.0, 0.0],
            },
            Row {
                x: vec![1.5, 0.4],
                y: vec![0.0, 0.0, 1.0],
            },
        ];
        let (_, g) = net.gradient(&w, &rows, &[0, 1]);
        for i in 0..w.len() {
            let mut up = w.clone();
            up[i] += 1e-6;
            let mut down = w.clone();
            down[i] -= 1e-6;
            let numeric =
                (net.error(&up, &rows, &[0, 1]) - net.error(&down, &rows, &[0, 1])) / 2e-6;
            assert!(
                (numeric - g[i]).abs() < 1e-8,
                "weight {i}: {numeric} vs {}",
                g[i]
            );
        }
    }

    #[test]
    fn one_batch_gradient_descent_step_by_hand() {
        let (net, w, rows) = tiny();
        let (_, g) = net.gradient(&w, &rows, &[0]);
        let s = Settings {
            scaled_conjugate: false,
            ..Settings::defaults()
        };
        let mut opt = Optimizer::Descent {
            eta: 0.4,
            dw: vec![0.0; 4],
        };
        let mut moved = w.clone();
        assert!(step(
            &net,
            &s,
            &mut moved,
            &mut opt,
            &rows,
            &[0],
            &[0],
            &mut 0
        ));
        // no momentum yet: w - 0.4 g lowers the error, so it is taken as it is
        for i in 0..4 {
            assert!((moved[i] - (w[i] - 0.4 * g[i])).abs() < 1e-15);
        }
        assert!(net.error(&moved, &rows, &[0]) < net.error(&w, &rows, &[0]));
    }

    #[test]
    fn one_scaled_conjugate_step_by_hand() {
        let (net, w, rows) = tiny();
        let (e0, g0) = net.gradient(&w, &rows, &[0]);
        let p: Vec<f64> = g0.iter().map(|g| -g).collect();
        let p2 = dot(&p, &p);
        // second-order information from the gradient a small step sigma0 / |p| along p
        let sigma = 0.00005 / p2.sqrt();
        let ahead: Vec<f64> = (0..4).map(|i| w[i] + sigma * p[i]).collect();
        let g1 = net.gradient(&ahead, &rows, &[0]).1;
        let delta: f64 =
            (0..4).map(|i| p[i] * (g1[i] - g0[i]) / sigma).sum::<f64>() + 0.0000005 * p2;
        assert!(delta > 0.0);
        let alpha = dot(&p, &p) / delta;
        let s = Settings::defaults();
        let mut opt = Optimizer::Scg {
            lambda: s.lambda0,
            lambda_bar: 0.0,
            r: p.clone(),
            p: p.clone(),
            success: true,
            delta: 0.0,
            steps: 0,
        };
        let mut moved = w.clone();
        assert!(step(
            &net,
            &s,
            &mut moved,
            &mut opt,
            &rows,
            &[0],
            &[0],
            &mut 0
        ));
        for i in 0..4 {
            assert!(
                (moved[i] - (w[i] + alpha * p[i])).abs() < 1e-12,
                "{moved:?}"
            );
        }
        assert!(net.error(&moved, &rows, &[0]) < e0);
    }

    #[test]
    fn removing_and_adding_hidden_units_keeps_the_other_weights() {
        let net = Net {
            sizes: vec![1, 2, 1],
            hidden: Activation::Tanh,
            output: Activation::Identity,
            groups: Vec::new(),
        };
        // hidden 1: (1, 2); hidden 2: (3, 4); output: (5, 6, 7)
        let w = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        assert_eq!(without(&net, &w, 0), vec![3.0, 4.0, 5.0, 7.0]);
        let mut u = || 0.75;
        let more = with_one_more(&net, &w, (0.0, 0.5), &mut u);
        assert_eq!(
            more,
            vec![1.0, 2.0, 3.0, 4.0, 0.25, 0.25, 5.0, 6.0, 7.0, 0.25]
        );
    }
}
