/* Minimal stub for compilation without libraw-dev installed */
#ifndef LIBRAW_H
#define LIBRAW_H

typedef struct {
    char make[128];
    char model[128];
} libraw_idata_t;

typedef struct {
    float iso_speed;
    float shutter;
    float aperture;
    float focal_len;
    long timestamp;
} libraw_other_t;

typedef struct {
    char Lens[128];
    char LensMake[128];
} libraw_lensinfo_t;

typedef struct {
    libraw_idata_t idata;
    libraw_other_t other;
    libraw_lensinfo_t lens;
} libraw_data_t;

#endif
