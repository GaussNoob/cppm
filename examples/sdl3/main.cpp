#include <SDL3/SDL.h>

int main() {
    return SDL_GetVersion() >= 3000000 ? 0 : 1;
}
