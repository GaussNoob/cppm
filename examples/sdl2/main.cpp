#define SDL_MAIN_HANDLED
#include <SDL.h>

int main() {
    SDL_version version{};
    SDL_GetVersion(&version);
    return version.major == 2 ? 0 : 1;
}
