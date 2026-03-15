#include <libraw/libraw.h>
#include <string.h>

void oxiraw_get_make(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->idata.make, buf_size - 1);
    buf[buf_size - 1] = '\0';
}

void oxiraw_get_model(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->idata.model, buf_size - 1);
    buf[buf_size - 1] = '\0';
}

float oxiraw_get_iso(libraw_data_t *data) {
    return data->other.iso_speed;
}

float oxiraw_get_shutter(libraw_data_t *data) {
    return data->other.shutter;
}

float oxiraw_get_aperture(libraw_data_t *data) {
    return data->other.aperture;
}

float oxiraw_get_focal_len(libraw_data_t *data) {
    return data->other.focal_len;
}

long long oxiraw_get_timestamp(libraw_data_t *data) {
    return (long long)data->other.timestamp;
}

void oxiraw_get_lens(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->lens.Lens, buf_size - 1);
    buf[buf_size - 1] = '\0';
}

void oxiraw_get_lens_make(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->lens.LensMake, buf_size - 1);
    buf[buf_size - 1] = '\0';
}
