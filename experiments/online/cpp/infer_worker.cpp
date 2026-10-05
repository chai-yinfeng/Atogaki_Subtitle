// Experimental PCM inference worker; replay/window policy lives in replay_asr.py.
#include "whisper.h"
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>

static std::string quote(const std::string &s) {
    std::string out = "\"";
    for (unsigned char c : s) {
        if (c == '"' || c == '\\') { out += '\\'; out += c; }
        else if (c == '\n') out += "\\n";
        else if (c == '\r') out += "\\r";
        else if (c == '\t') out += "\\t";
        else if (c < 32) {
            const char *hex = "0123456789abcdef";
            out += "\\u00"; out += hex[c >> 4]; out += hex[c & 15];
        } else out += c;
    }
    return out + '"';
}
int main(int argc, char **argv) {
    if (argc != 3) { std::cerr << "usage: infer-worker model language\n"; return 2; }
    auto cp = whisper_context_default_params();
    cp.use_gpu = true; cp.flash_attn = true;
    whisper_context *ctx = whisper_init_from_file_with_params(argv[1], cp);
    if (!ctx) return 3;
    std::cout << "{\"ready\":true}" << std::endl;
    uint32_t count;
    while (std::cin.read(reinterpret_cast<char *>(&count), sizeof(count))) {
        if (!count || count > 16000 * 31) { whisper_free(ctx); return 4; }
        std::vector<float> audio(count);
        if (!std::cin.read(reinterpret_cast<char *>(audio.data()), count * sizeof(float))) {
            whisper_free(ctx); return 5;
        }
        auto p = whisper_full_default_params(WHISPER_SAMPLING_GREEDY);
        p.n_threads = 4; p.language = argv[2];
        p.temperature = 0.0f; p.temperature_inc = 0.0f; p.greedy.best_of = 1;
        p.single_segment = true; p.no_context = true; p.no_timestamps = true;
        p.print_progress = false; p.print_realtime = false; p.print_timestamps = false;
        if (whisper_full(ctx, p, audio.data(), count)) { whisper_free(ctx); return 6; }
        std::string text;
        for (int i = 0; i < whisper_full_n_segments(ctx); ++i)
            text += whisper_full_get_segment_text(ctx, i);
        std::cout << "{\"text\":" << quote(text) << "}" << std::endl;
    }
    whisper_free(ctx);
}
