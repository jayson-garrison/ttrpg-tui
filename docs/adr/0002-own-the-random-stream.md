# Own the random stream instead of using rand's range API

Tests pin exact outputs for a given seed, but `rand` may change how it maps raw bits to a range between versions. We expand the user's seed to the ChaCha8 key with our own SplitMix64 step and map `next_u32` to `1..=sides` with our own rejection sampler, so a dependency upgrade can never silently change results. Cost: about thirty lines we must keep correct (covered by golden-vector and distribution tests). See D-04.
