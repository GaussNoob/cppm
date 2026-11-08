// Integration fixture, not an obfuscator: a visible literal replacement proves
// that the plugin actually ran in Clang's compilation pipeline.
#include <llvm/IR/Constants.h>
#include <llvm/IR/Module.h>
#include <llvm/Passes/PassBuilder.h>
#if __has_include(<llvm/Plugins/PassPlugin.h>)
#include <llvm/Plugins/PassPlugin.h>
#else
#include <llvm/Passes/PassPlugin.h>
#endif
using namespace llvm;

struct FixturePass : PassInfoMixin<FixturePass> {
    PreservedAnalyses run(Module &module, ModuleAnalysisManager &) {
        for (auto &global : module.globals()) {
            if (!global.hasInitializer()) continue;
            auto *value = dyn_cast<ConstantDataArray>(global.getInitializer());
            if (value && value->isCString() && value->getAsCString() == "plugin-before") {
                global.setInitializer(ConstantDataArray::getString(module.getContext(), FIXTURE_REPLACEMENT));
            }
        }
        return PreservedAnalyses::none();
    }
};

extern "C" LLVM_ATTRIBUTE_WEAK PassPluginLibraryInfo llvmGetPassPluginInfo() {
    return {LLVM_PLUGIN_API_VERSION, "cppm-integration-fixture", "1",
        [](PassBuilder &builder) {
            builder.registerPipelineStartEPCallback(
                [](ModulePassManager &manager, OptimizationLevel) {
                    manager.addPass(FixturePass());
                });
        }};
}
