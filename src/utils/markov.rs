use std::{
    collections::HashMap,
    iter::{Skip, FromIterator},
};
use itertools::{
    multizip,
    Zip,
};
use rand;


fn to_triplets<I>(iter: I) -> Zip<(I, Skip<I>, Skip<I>)>
    where I: Iterator + Clone,
{
    let b = iter.clone();
    let c = iter.clone();

    multizip((iter, b.skip(1), c.skip(2)))
}


#[derive(Hash, Eq, PartialEq, Copy, Clone, Debug)]
enum MarkovEntry<'a> {
    Start,
    Word(&'a str),
    End,
}

// Only lives in the lifetime of it's input
#[derive(Default)]
pub struct MChain<'a> {
    map: HashMap<(MarkovEntry<'a>, MarkovEntry<'a>), HashMap<MarkovEntry<'a>, f32>>,
}


impl<'a> MChain<'a> {
    pub fn add_string(&mut self, s: &'a str) {

        let sentences = s.split(|c| ".!?\n".contains(c));

        for sentence in sentences {
            let mut split = vec![MarkovEntry::Start];
            split.extend(sentence.split_whitespace().map(MarkovEntry::Word));
            split.push(MarkovEntry::End);

            let first = split[1];

            self.insert_triplet((MarkovEntry::Start, MarkovEntry::Start, first));

            for t in to_triplets(split.into_iter()) {
                self.insert_triplet(t);
            }
        }
    }

    fn insert_triplet(&mut self, t: (MarkovEntry<'a>, MarkovEntry<'a>, MarkovEntry<'a>)) {
        let key = (t.0, t.1);
        let val = t.2;

        let entry = self.map.entry(key).or_insert_with(HashMap::new);

        *entry.entry(val).or_insert(1.0) *= 1.1;
    }

    pub fn generate_string(&self, limit: usize, minimum: usize) -> Option<String> {
        use std::u32;
        use rand::distributions::{Weighted, WeightedChoice, Distribution};

        let mut res = String::new();
        let mut state = (MarkovEntry::Start, MarkovEntry::Start);

        let mut rng = rand::thread_rng();

        for _ in 0..limit {
            if let Some(r) = self.map.get(&state) {
                let sum: usize = r.iter().map(|(_, &v)| v as usize).sum();

                // welp
                if sum == 0 {
                    break;
                }

                let mut dist: Vec<_> = r.iter().map(|(k, &v)| Weighted { weight: v as u32, item: k}).collect();

                if sum > u32::MAX as usize {
                    // If the sum of each value is greater than a u32 size, we need to shrink our weights
                    let ratio = (sum / u32::MAX as usize) as u32;
                    for mut v in &mut dist {
                        v.weight /= ratio;
                    }
                }

                let wc = WeightedChoice::new(&mut dist);

                let next = wc.sample(&mut rng);
                match next {
                    MarkovEntry::Word(w) => { res.push_str(" "); res.push_str(w); },
                    MarkovEntry::End     => break,
                    MarkovEntry::Start   => unreachable!(),
                }
                state = (state.1, *next);
            }
        }

        if res.is_empty() {
            return None;
        }

        if res.chars().filter(|&c| c.is_alphanumeric()).count() < minimum {
            return None;
        }

        Some(res)
    }
}


impl<'a> Extend<&'a str> for MChain<'a> {
    fn extend<I: IntoIterator<Item=&'a str>>(&mut self, iter: I) {
        for elem in iter {
            self.add_string(elem);
        }
    }
}


impl<'a> FromIterator<&'a str> for MChain<'a> {
    fn from_iter<I: IntoIterator<Item=&'a str>>(iter: I) -> Self {
        let mut c = Self::default();
        c.extend(iter);
        c
    }
}


impl<'a> Extend<&'a String> for MChain<'a> {
    fn extend<I: IntoIterator<Item=&'a String>>(&mut self, iter: I) {
        for elem in iter {
            self.add_string(elem);
        }
    }
}


impl<'a> FromIterator<&'a String> for MChain<'a> {
    fn from_iter<I: IntoIterator<Item=&'a String>>(iter: I) -> Self {
        let mut c = Self::default();
        c.extend(iter);
        c
    }
}
