#ifndef WASMER_SVM_H
#define WASMER_SVM_H

#include "wasmer.h"
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {

} wasmer_import_object_t;

/**
 * Returns a pointer to the `svm context node_data`.
 * It will be used by the node vmcalls implementation.
 */
void *wasmer_svm_instance_context_node_data_get(const wasmer_instance_context_t *ctx);

/**
 * Returns a pointer to register internal bytes array
 */
uint8_t *wasmer_svm_register_get(const wasmer_instance_context_t *ctx, uint32_t reg_idx);


/**
  Copies `bytes_len` bytes from raw pointer `bytes` into `wasmer svm` register indexed `reg_idx`.
 */
void wasmer_svm_register_set(const wasmer_instance_context_t *ctx,
                             uint32_t reg_idx,
                             uint8_t *bytes_ptr,
                             uint32_t bytes_len);

/**
 * Creates a new Import object
 * Returns `wasmer_result_t::WASMER_OK` upon success.
 * Returns `wasmer_result_t::WASMER_ERROR` upon failure. Use `wasmer_last_error_length`
 * and `wasmer_last_error_message` to get an error message.
 */
wasmer_result_t wasmer_svm_import_object(wasmer_import_object_t** import_object,
                                         void *addr_ptr,
                                         uint32_t max_pages,
                                         uint32_t max_page_slices,
                                         void *node_data,
                                         wasmer_import_t *imports,
                                         uint32_t imports_len);


/**
 * Given a compiler wasmer module (param `module`) and a ready-made import object (param `import_object`),
 * instantiates a new wasmer instance.
 * The instance is returned via the param `instance` (that's why it's of type `wasmer_instance_t**`)
 *
 * Returns `wasmer_result_t::WASMER_OK` upon success.
 * Returns `wasmer_result_t::WASMER_ERROR` upon failure. Use `wasmer_last_error_length`
 * and `wasmer_last_error_message` to get an error message.
 */
wasmer_result_t wasmer_svm_module_instantiate(wasmer_instance_t** instance,
                                              wasmer_module_t* module,
                                              wasmer_import_object_t* import_object);


/**
 * Frees memory of the given ImportObject
 */
void wasmer_import_object_destroy(wasmer_import_object_t* import_object);

#endif /* WASMER_SVM_H */
