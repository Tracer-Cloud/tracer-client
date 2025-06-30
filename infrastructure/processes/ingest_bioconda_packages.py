#!/usr/bin/env python3
"""
Script to parse meta.yaml files from bioconda recipes and extract package information.
"""

import argparse
import math
import re
import shlex
import shutil
import subprocess
import sys
import textwrap
from pathlib import Path

import yaml
from jinja2 import Environment

VERSION_RE = re.compile(r'{%\s*set version\s*=\s*["\']([^"\']+)["\']')


def find_meta_file(d: Path) -> Path:
    """
    Find the meta.yaml file in a recipe directory.
    """
    while True:
        meta_file = d / "meta.yaml"
        if meta_file.exists():
            return meta_file

        subdirs = [d for d in d.iterdir() if d.is_dir()]
        if len(subdirs) == 1:
            d = subdirs[0]
        else:
            break


def noop(*args, **kwargs):
    return ""


def invalid_command(arg: str) -> bool:
    return (
        arg.startswith("-") or arg[0] in "{}<>&|;'\"$=" or arg in ("grep", "/dev/null")
    )


def resolve_commands(name: str, commands: str, executable_packages: list) -> bool:
    resolved_commands = set()
    for command in commands:
        args = [arg for arg in shlex.split(command) if not invalid_command(arg)]
        if len(args) == 0:
            continue
        found = False
        if len(args) > 1:
            for arg in args:
                if name.lower() in arg.lower():
                    resolved_commands.add(arg)
                    found = True
                    break
        if not found:
            resolved_commands.add(args[0])

    if len(resolved_commands) == 0:
        return False

    for command in resolved_commands:
        if name == command:
            rule_name = f"{command} process"
        else:
            rule_name = f"{name} {command} process"
        if ".py" in command:
            condition = {
                "and": [
                    {"process_name_contains": "python"},
                    {"min_args": 1},
                    {"first_arg_is": command},
                ]
            }
        else:
            condition = {
                "process_name_is": command,
            }
        executable_packages.append(
            {"rule_name": rule_name, "display_name": command, "condition": condition}
        )

    return True


def parse_meta_yaml(
    package: str,
    file_path: Path,
    executable_packages: list,
    importable_packages: list,
    ambiguous_packages: list,
    errors: list[str],
    timeout: int,
) -> None:
    """
    Parse a meta.yaml file and extract package information.

    Args:
        file_path: Path to the meta.yaml file

    Returns:
        Dictionary containing extracted information or None if parsing failed
    """

    try:
        # Read the entire file content
        with open(file_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Create Jinja2 environment that ignores undefined variables
        env = Environment()

        # Render the template with Jinja2, ignoring undefined variables
        rendered_content = env.from_string(content).render(
            pin_subpackage=noop,
            compiler=noop,
            environ="",
            pin_compatible=noop,
            cdt=noop,
            stdlib=noop,
            os={"environ": {"get": ""}},
        )

        # Parse the rendered YAML
        yaml_data = yaml.safe_load(rendered_content)

        if not yaml_data:
            errors.append(
                {
                    "package": package,
                    "message": f"Error parsing meta.yaml for package {package}",
                    "type": "error",
                }
            )
            return

        # Get package name
        if "package" in yaml_data and "name" in yaml_data["package"]:
            name = yaml_data["package"]["name"]
        else:
            errors.append(
                {
                    "package": name,
                    "message": f"No package name for package {package}",
                    "type": "error",
                }
            )
            return

        # Get version from YAML data
        if "package" in yaml_data and "version" in yaml_data["package"]:
            version = yaml_data["package"]["version"]
        else:
            errors.append(
                {
                    "package": name,
                    "message": f"No version found for package {package}",
                    "type": "warning",
                }
            )
            version = None

        # Get test commands
        if "test" in yaml_data and yaml_data["test"] is not None:
            if "commands" in yaml_data["test"]:
                env = f"pixi-{name}"
                if version is None:
                    spec = name
                else:
                    spec = f"{name}={version}"
                test_commands = yaml_data["test"]["commands"]
                command = None
                try:
                    # check all the commands in an environment where the package is installed;
                    # if exactly one works, use that as the command, otherwise store in a separate
                    # list for manual resolution
                    env_dir = Path(env)
                    env_dir.mkdir(parents=True, exist_ok=True)
                    manifest_file = env_dir / "pixi.toml"
                    with open(manifest_file, "w", encoding="utf-8") as f:
                        f.write(
                            textwrap.dedent(
                                f"""
                                [workspace]
                                name = "{env}"
                                channels = ["conda-forge", "bioconda"]
                                platforms = ["linux-64"]
                                channel-priority = "disabled"
                                """
                            )
                        )

                    proc = subprocess.run(
                        f"pixi add --manifest-path {env} {spec}",
                        shell=True,
                        capture_output=True,
                    )
                    if proc.returncode != 0:
                        errors.append(
                            {
                                "package": name,
                                "message": f"Error creating environment for package {name}: {proc.stderr.decode('utf-8')}",
                                "type": "error",
                            }
                        )
                        return

                    successful_commands = []
                    for i, command in enumerate(test_commands):
                        print(
                            f"Executing {name} command {i} of {len(test_commands)}: {command}"
                        )
                        pixi_command = f"pixi run --manifest-path {env} {command}"
                        try:
                            proc = subprocess.run(
                                pixi_command,
                                shell=True,
                                capture_output=True,
                                timeout=timeout,
                            )
                            if proc.returncode == 0:
                                print("  success")
                                successful_commands.append(command)
                            else:
                                print("  failure")
                        except subprocess.TimeoutExpired:
                            print("  timeout")
                            errors.append(
                                {
                                    "package": name,
                                    "message": "Timed out executing command {pixi_command}",
                                    "type": "warning",
                                }
                            )
                finally:
                    if Path(env).exists():
                        shutil.rmtree(env)

                if len(successful_commands) > 0 and resolve_commands(
                    name, successful_commands, executable_packages
                ):
                    return

                ambiguous_packages.append(
                    {
                        "name": name,
                        "version": version,
                        "test_commands": test_commands,
                        "successful_commands": successful_commands,
                        "recipe_dir": package,
                    }
                )
            elif "imports" in yaml_data["test"]:
                importable_packages.append(
                    {
                        "name": name,
                        "version": version,
                        "test_imports": yaml_data["test"]["imports"],
                        "recipe_dir": package,
                    }
                )
            else:
                errors.append(
                    {
                        "package": name,
                        "message": f"No test commands or imports found for package {package}",
                        "type": "error",
                    }
                )
        else:
            errors.append(
                {
                    "package": name,
                    "message": f"No test section found for package {package}",
                    "type": "error",
                }
            )

    except yaml.YAMLError as e:
        errors.append(
            {
                "package": name,
                "message": f"YAML parsing error for package {package}: {e}",
                "type": "error",
            }
        )
    except Exception as e:
        errors.append(
            {
                "package": name,
                "message": f"Unexpected error for package {package}: {e}",
                "type": "error",
            }
        )


def main():
    """
    Main function to process all recipes.
    """
    print("A")
    parser = argparse.ArgumentParser(
        description="Parse bioconda recipes meta.yaml files"
    )
    parser.add_argument(
        "-d",
        "--recipes-dir",
        type=Path,
        help="Path to recipes directory",
        default=Path("recipes"),
    )
    parser.add_argument(
        "-o",
        "--output-dir",
        type=Path,
        help="Path to output directory",
        default=None,
    )
    parser.add_argument(
        "-t",
        "--timeout",
        type=int,
        help="Timeout in seconds for pixi commands",
        default=20,
    )
    parser.add_argument("chunk", type=int, help="Current chunk number (0-based)")
    parser.add_argument("total_chunks", type=int, help="Total number of chunks")
    args = parser.parse_args()
    print("B")
    recipes_dir = args.recipes_dir

    if not recipes_dir.exists():
        sys.exit("Recipes directory not found!")
    print("C")
    recipe_dirs = sorted([d for d in recipes_dir.iterdir() if d.is_dir()])

    chunk = args.chunk
    total_chunks = args.total_chunks
    num_dirs = len(recipe_dirs)
    if num_dirs <= total_chunks:
        chunk_size = 1
    else:
        chunk_size = math.ceil(num_dirs / total_chunks)

    start_index = chunk * chunk_size
    end_index = min(start_index + chunk_size, num_dirs)
    if start_index >= end_index:
        return

    print(
        f"Processing chunk {chunk}: {start_index} to {end_index} of {num_dirs} directories"
    )

    meta_files = []
    missing_meta_yaml = []
    for d in recipe_dirs[start_index:end_index]:
        meta_file = find_meta_file(d)
        if meta_file is None:
            missing_meta_yaml.append(d.name)
        else:
            meta_files.append((d, meta_file))

    executable_packages = []
    importable_packages = []
    ambiguous_packages = []
    errors = []

    # Iterate through all subdirectories in recipes
    for i, (recipe_dir, meta_file) in enumerate(meta_files):
        print(f"Processing {i} of {len(meta_files)}")
        parse_meta_yaml(
            recipe_dir.name,
            meta_file,
            executable_packages,
            importable_packages,
            ambiguous_packages,
            errors,
            args.timeout,
        )

    if args.output_dir is None:
        output_dir = Path(".")
    else:
        output_dir = args.output_dir

    if not output_dir.exists():
        output_dir.mkdir(parents=True, exist_ok=True)

    # Write results to YAML file
    output_file = output_dir / f"bioconda.rules.{chunk}.yml"
    importable_file = output_dir / f"bioconda.importable.{chunk}.yml"
    ambiguous_packages_file = output_dir / f"bioconda.ambiguous.{chunk}.yml"
    errors_file = output_dir / f"bioconda.errors.{chunk}.yml"
    missing_meta_yaml_file = output_dir / f"missing_meta_yaml.{chunk}.txt"
    try:
        with open(output_file, "w", encoding="utf-8") as f:
            yaml.dump(
                {"rules": executable_packages}, f, default_flow_style=False, indent=2
            )
        with open(importable_file, "w", encoding="utf-8") as f:
            yaml.dump(
                {"packages": importable_packages}, f, default_flow_style=False, indent=2
            )
        with open(ambiguous_packages_file, "w", encoding="utf-8") as f:
            yaml.dump(
                {"packages": ambiguous_packages}, f, default_flow_style=False, indent=2
            )
        with open(errors_file, "w", encoding="utf-8") as f:
            yaml.dump({"errors": errors}, f, default_flow_style=False, indent=2)
        with open(missing_meta_yaml_file, "w", encoding="utf-8") as f:
            f.write("\n".join(missing_meta_yaml))
    except Exception as e:
        sys.exit(f"Error writing output file: {e}")

    print(f"Missing meta.yaml for {len(missing_meta_yaml)} packages")
    print(f"Successfully processed {len(executable_packages)} executable packages")
    print(f"Successfully processed {len(importable_packages)} importable packages")
    print(f"Unresolved packages: {len(ambiguous_packages)}")
    print(f"Encountered errors/warnings in {len(errors)} packages")
    print(f"Executable packages written to {output_file}")
    print(f"Importable packages written to {importable_file}")
    print(f"Unresolved packages written to {ambiguous_packages_file}")
    print(f"Packages with missing meta.yaml written to {missing_meta_yaml_file}")
    print(f"Errors/warnings written to {errors_file}")


if __name__ == "__main__":
    main()
