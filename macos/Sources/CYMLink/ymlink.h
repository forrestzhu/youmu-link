#ifndef YMLINK_H
#define YMLINK_H

// Returned strings belong to Rust; release them exactly once with ymlink_string_free.
void *ymlink_create(const char *state_dir, const char *device_name);
char *ymlink_snapshot(void *handle);
char *ymlink_command(void *handle, const char *json);
void ymlink_string_free(char *string);
void ymlink_destroy(void *handle);

#endif
