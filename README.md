# shuttle-env-vars


[![license](https://img.shields.io/crates/l/shuttle-env-vars?style=for-the-badge)](https://github.com/robertohuertasm/shuttle-env-vars/blob/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/shuttle-env-vars?style=for-the-badge)](https://crates.io/crates/shuttle-env-vars)
[![docs.rs](https://img.shields.io/docsrs/shuttle-env-vars?style=for-the-badge)](https://docs.rs/shuttle-env-vars)

A library to use `.env` files in your [Shuttle](https://shuttle.rs) projects.

[Shuttle Secrets](https://docs.shuttle.rs/resources/shuttle-secrets) is ok, but sometimes you need to use environment variables so they can be used in other crates.

You could use [Shuttle Secrets](https://docs.shuttle.rs/resources/shuttle-secrets) and then iterate over the secrets and set them as environment variables, or you can use this crate.

## Usage

This crate leverages [Shuttle Static Folder](https://docs.shuttle.rs/resources/shuttle-static-folder) to read the `.env` files and set the environment variables.

Add the following to your `Cargo.toml`:

```toml
[dependencies]
shuttle-env-vars = "0.21.0"
```

Then create a `.env` file in the folder you want to use. Note that the name of the file can be anything.

Once you have created you `.env` file you have to add the following argument to your Shuttle `main` function:

```rust
#[shuttle_runtime::main]
async fn main(
    #[shuttle_env_vars::EnvVars(folder = "name_of_your_folder", env_prod = "name_of_your_env_file")] _env_folder: PathBuf,
) -> __ { ... }
```

All the environment variables defined in your `.env` file will be automatically set.

Note that we're using `PathBuf` as the type of the argument. This is because of a current restriction in Shuttle when dealing with custom resources. You can ignore this argument if you don't plan to do anything. Otherwise, it will return the path to the folder containing your `.env` files if any.


### Local mode

If you want to use a particular `.env` file in local mode, you can use the `env_local` parameter:

```rust
#[shuttle_runtime::main]
async fn main(
    #[shuttle_env_vars::EnvVars(folder = "name_of_your_folder", env_prod = "name_of_your_env_file", env_local = "your_path_to_your_local_env_file")] _env_folder_¡: PathBuf,
) -> __ { ... }
```

 When executing locally, both `folder` and `env_prod` will be ignored and only `env_local` will be used. It's **important** to note that `env_local` is a path to a file, not a file name.


 ### Defaults

 All the arguments are optional:

 - *folder*: This is the folder containing your `.env` files. It will default to `.env`.
 - *env_prod*: Filename of the `.env` file you will use in production. It will default to `.env`.
 - *env_local*: File path of the `.env` file you will use in local mode. This is an optional parameter and it defaults to `None`.

## Ignoring your .env files

Typically, the `.env` files are not committed to your repository and are ignored.

That's a good practice, but won't work for Shuttle as it won't upload the files that are present in the `.gitignore` file.

To overcome this, you can take a look at the `Caveats` section in [Shuttle Static Folder](https://docs.shuttle.rs/resources/shuttle-static-folder#caveats) because that's precisely what we need to do.

We will need to create a file called `.ignore` and explictly include our `.env` files.

For example, if you have a `.gitignore` file like this:

```sh
dist/
static/
target/
.env*
```

Then, in order to include the `.env` folder and files in the final archive, you’ll have to create a `.ignore` file like this:

```sh
!.env*
```

That will do the trick!

## CI/CD

If you are using Shuttle in your CI/CD pipeline, you will need to add the `.env` files to the final archive.

One solution to avoid having to commit the `.env` file, is to store the `.env` file content as a secret in your CI/CD provider and then create the `.env` file in the CI/CD pipeline.

For example, if you are using GitHub Actions, you can do something like this:

```yaml
 - name: Set ENV vars file
   shell: bash
   run: |
    mkdir .env
    echo "${{ secrets.PROD_ENV_VARS }}" > .env/.env-prod
```


