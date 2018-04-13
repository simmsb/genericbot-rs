use std::{
    collections::HashMap,
    iter::Skip,
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


#[derive(Hash, Eq, PartialEq, Clone, Debug)]
enum MarkovEntry<'a> {
    Start,
    Word(&'a str),
    End,
}

// Only lives in the lifetime of it's input
pub struct MChain<'a> {
    map: HashMap<(MarkovEntry<'a>, MarkovEntry<'a>), HashMap<MarkovEntry<'a>, u32>>,
}


impl<'a> MChain<'a> {
    pub fn new() -> MChain<'a> {
        MChain {
            map: HashMap::new(),
        }
    }

    pub fn add_string(&mut self, s: &'a str) {

        let sentences = s.split(|c| ".!?\n".contains(c));

        for sentence in sentences {
            let mut split = vec![MarkovEntry::Start];
            split.extend(sentence.split_whitespace().map(MarkovEntry::Word));
            split.push(MarkovEntry::End);

            let first = split[1].clone();

            self.insert_triplet((MarkovEntry::Start, MarkovEntry::Start, first));

            for t in to_triplets(split.into_iter()) {
                // println!("inserting triplet: {:?}", &t);
                self.insert_triplet(t);
            }
        }
    }

    fn insert_triplet(&mut self, t: (MarkovEntry<'a>, MarkovEntry<'a>, MarkovEntry<'a>)) {
        let key = (t.0, t.1);
        let val = t.2;

        let entry = self.map.entry(key).or_insert_with(HashMap::new);

        *entry.entry(val).or_insert(0) += 1;
    }

    pub fn generate_string(&self, limit: usize) -> Option<String> {
        use rand::distributions::{Weighted, WeightedChoice, IndependentSample};

        let mut res = String::new();
        let mut state = (MarkovEntry::Start, MarkovEntry::Start);

        let mut rng = rand::thread_rng();

        // println!("current map: {:?}", &self.map);

        for _ in 0..limit {
            if let Some(r) = self.map.get(&state) {
                let mut dist: Vec<_> = r.iter().map(|(k, &v)| Weighted { weight: v, item: k}).collect();
                let wc = WeightedChoice::new(&mut dist);
                let next = wc.ind_sample(&mut rng);
                // println!("current state: {}", &res);
                match next {
                    MarkovEntry::Word(w) => { res.push_str(" "); res.push_str(w); },
                    MarkovEntry::End     => return Some(res),
                    MarkovEntry::Start   => unreachable!(),
                }
                state = (state.1, next.clone());
            }
        }

        if res.is_empty() {
            return None;
        }

        return Some(res);
    }
}
