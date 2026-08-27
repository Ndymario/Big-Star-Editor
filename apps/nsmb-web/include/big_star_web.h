#ifndef BIG_STAR_WEB_H
#define BIG_STAR_WEB_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Stable, browser-neutral ABI. Pointers returned by Big Star remain owned by
// the session and are valid until the next call that mutates that session.
enum bse_collection {
    BSE_OBJECT = 1,
    BSE_SPRITE = 2,
    BSE_ENTRANCE = 3,
    BSE_VIEW = 4,
    BSE_ZONE = 5,
    BSE_PATH = 6,
    BSE_PROGRESS_PATH = 7,
    BSE_OBJECT_BANK = 8,
    BSE_REFERENCES = 9
};

uint32_t bse_api_version(void);
const char* bse_last_error(void);

uint32_t bse_level_open(const uint8_t* course, uint32_t course_size,
                        const uint8_t* bgdat, uint32_t bgdat_size);
void bse_level_close(uint32_t handle);

// A JSON snapshot for presentation. Numeric field order is documented in the
// web README and is also named in the JSON object, so JavaScript does not need
// C++ layout knowledge.
const char* bse_level_snapshot(uint32_t handle);

// Field numbers are zero-based in the order emitted by the snapshot. Paths
// expose id and unknown; their nodes are currently read-only in the web shell.
int bse_level_set(uint32_t handle, uint32_t collection, uint32_t index,
                  uint32_t field, double value);
int bse_level_add(uint32_t handle, uint32_t collection);
int bse_level_remove(uint32_t handle, uint32_t collection, uint32_t index);
int bse_level_undo(uint32_t handle);
int bse_level_redo(uint32_t handle);

// Serialize once, then read both buffers. Data pointers remain valid until the
// session is mutated or serialized again.
int bse_level_serialize(uint32_t handle);
const uint8_t* bse_level_course_data(uint32_t handle);
uint32_t bse_level_course_size(uint32_t handle);
const uint8_t* bse_level_bgdat_data(uint32_t handle);
uint32_t bse_level_bgdat_size(uint32_t handle);

#ifdef __cplusplus
}
#endif

#endif
