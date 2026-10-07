#include <woff2/decode.h>
#include <woff2/encode.h>

#ifdef __cplusplus
extern "C" {
#endif

int ConvertTTFToWOFF2(
    const uint8_t *data,
    size_t length,
    uint8_t *result,
    size_t *result_length,
    const char* extended_metadata,
    size_t extended_metadata_length,
    int brotli_quality,
    int allow_transforms
) {
    std::string extended_metadata_copy(extended_metadata, extended_metadata_length);
    woff2::WOFF2Params params;
    params.extended_metadata = extended_metadata_copy;
    params.brotli_quality = brotli_quality;
    params.allow_transforms = allow_transforms != 0;
    return woff2::ConvertTTFToWOFF2(data, length, result, result_length, params) ? 1 : 0;
}

int ConvertWOFF2ToTTF(
    uint8_t *result,
    size_t result_length,
    const uint8_t *data,
    size_t length
) {
    return woff2::ConvertWOFF2ToTTF(result, result_length, data, length) ? 1 : 0;
}

size_t ComputeTTFToWOFF2Size(
    const uint8_t *data,
    size_t length,
    const char* extended_metadata,
    size_t extended_metadata_length
) {
    std::string metadata(extended_metadata, extended_metadata_length);
    return woff2::MaxWOFF2CompressedSize(data, length, metadata);
}

size_t ComputeWOFF2ToTTFSize(
    const uint8_t *data,
    size_t length
) {
    return woff2::ComputeWOFF2FinalSize(data, length);
}

#ifdef __cplusplus
}
#endif
