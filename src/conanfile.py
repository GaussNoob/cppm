"""Generated consumer recipe. Placeholder values are replaced by the CLI."""

import json
import os

from conan import ConanFile
from conan.tools.env import VirtualRunEnv
from conan.tools.files import save


class CppmProject(ConanFile):
    settings = "os", "compiler", "arch", "build_type"
    requires = @CPPM_REQUIRES@
    default_options = @CPPM_OPTIONS@
    generators = "CMakeDeps", "CMakeToolchain"

    def generate(self):
        runenv = VirtualRunEnv(self)
        runenv.generate()
        variables = runenv.vars()
        # Keep inherited values as references. Do not persist the caller's PATH,
        # credentials or other environment values in the generated project files.
        values = {
            name: value if variables.get(name) is not None else None
            for name, value in variables.items(
                variable_reference="__CPPM_INHERIT_{name}__"
            )
        }
        save(
            self,
            os.path.join(self.generators_folder, "cppm-runtime.json"),
            json.dumps({"format": 1, "variables": values}, ensure_ascii=False),
        )
