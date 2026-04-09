AgX simulates film grain by convolving white noise with a Gaussian kernel
whose sigma is proportional to the configured grain size, then modulating
the result by per-pixel luminance to mimic the way film grain is more
pronounced in midtones than in deep shadows or bright highlights.

The current algorithm replaced an older frequency-based approach. See the
project's design history for the trade-offs and the reference photographs
that informed the parameter choices.
