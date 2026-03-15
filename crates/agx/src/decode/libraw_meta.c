#include <libraw/libraw.h>
#include <string.h>

void agx_get_make(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->idata.make, buf_size - 1);
    buf[buf_size - 1] = '\0';
}

void agx_get_model(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->idata.model, buf_size - 1);
    buf[buf_size - 1] = '\0';
}

float agx_get_iso(libraw_data_t *data) {
    return data->other.iso_speed;
}

float agx_get_shutter(libraw_data_t *data) {
    return data->other.shutter;
}

float agx_get_aperture(libraw_data_t *data) {
    return data->other.aperture;
}

float agx_get_focal_len(libraw_data_t *data) {
    return data->other.focal_len;
}

long long agx_get_timestamp(libraw_data_t *data) {
    return (long long)data->other.timestamp;
}

void agx_get_lens(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->lens.Lens, buf_size - 1);
    buf[buf_size - 1] = '\0';
}

void agx_get_lens_make(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->lens.LensMake, buf_size - 1);
    buf[buf_size - 1] = '\0';
}
