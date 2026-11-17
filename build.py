"""Zip each plugin folder here into <id>.zip, with the plugin in one top-level folder."""
import os
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
for name in sorted(os.listdir(HERE)):
    folder = os.path.join(HERE, name)
    if not os.path.isfile(os.path.join(folder, "plugin.json")):
        continue
    target = os.path.join(HERE, f"{name}.zip")
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as z:
        for file in sorted(os.listdir(folder)):
            z.write(os.path.join(folder, file), f"{name}/{file}")
    print(target, os.path.getsize(target))
