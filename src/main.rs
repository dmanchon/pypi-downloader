use core::fmt;

use anyhow::{Context, Result};
use reqwest;
use tokio::fs;

#[derive(Debug)]
struct Package {
    file_name: String,
    url: String,
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file_name, self.url)
    }
}

async fn list_versions(url: String, base_url: &String) -> Result<Vec<Package>> {
    let mut result = Vec::new();

    let body = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .context("Failed to request")?
        .text()
        .await
        .context("Failed to retrieve body as text")?;

    let dom = tl::parse(body.as_str(), tl::ParserOptions::default())
        .with_context(|| format!("Fail to parse HTML body of {}", url))?;
    let parser = dom.parser();
    for link in dom
        .query_selector("a[href]")
        .context("Fail to query 'a[ref]' tags")?
    {
        match link.get(parser) {
            Some(tag) => {
                if let Some(att) = tag
                    .as_tag()
                    .context("Fail to retrieve tag '<a>'")?
                    .attributes()
                    .get("href")
                {
                    if let Some(href) = att {
                        let pkg_url = format!("{}{}", base_url, href.as_utf8_str());
                        let name = tag.inner_text(parser);
                        result.push(Package {
                            file_name: name.to_string(),
                            url: pkg_url.to_string(),
                        });
                    }
                }
            }
            None => (),
        };
    }

    Ok(result)
}

async fn list_packages(url: &String, base_path: &str) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let path = format!("{}/packages.html", base_path);
    let u = format!("{}/simple/", url);
    println!("{:#?}", u);
    let body = match fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(_) => {
            let content = reqwest::Client::new()
                .get(u)
                .send()
                .await
                .context("Fail to request")?
                .text()
                .await
                .context("Fail to retrieve body as text")?;
            fs::write(&path, &content)
                .await
                .context("Fail to create the file")?;
            content
        }
    };

    let dom = tl::parse(body.as_str(), tl::ParserOptions::default())
        .with_context(|| format!("Fail to parse HTML body of {}", url))?;
    let parser = dom.parser();
    for link in dom
        .query_selector("a[href]")
        .context("Fail to query 'a[href]' tags")?
    {
        match link.get(parser) {
            Some(tag) => {
                if let Some(att) = tag
                    .as_tag()
                    .context("Fail to retrieve tag '<a>'")?
                    .attributes()
                    .get("href")
                {
                    if let Some(href) = att {
                        result.push(format!("{}/{}", url, href.as_utf8_str()));
                    }
                }
            }
            None => (),
        }
    }
    Ok(result)
}

async fn download_pkg(pkg: Package, base_path: &str) -> Result<()> {
    let path = format!("{}/{}", base_path, pkg.file_name);

    if std::path::Path::new(&path).is_file() {
        println!("{:#?} already exists. Skipping.", pkg);
    } else {
        let content = reqwest::Client::new()
            .get(pkg.url)
            .send()
            .await
            .context("Fail to request")?
            .bytes()
            .await
            .context("Fail to retrieve body as bytes")?;
        fs::write(&path, &content)
            .await
            .with_context(|| format!("Fail to create the file {}", &path))?;
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let url = std::env::args().nth(1).clone().expect("'pypi url' param missing");
    let str_path = std::env::args().nth(2).clone().expect("'path' param missing");
    let relpath = std::path::PathBuf::from(str_path);
    let path = relpath.as_path().to_string_lossy();

    match list_packages(&url, &path).await {
        Err(e) => print!("Fail to parse simple pypi: {:#?}", e),
        Ok(list) => {
            for package_link in list {
                match list_versions(package_link, &url).await {
                    Ok(urls) => {
                        for pkg in urls {
                            println!("Downloading {} ...", pkg.url);
                            if let Err(e) = download_pkg(pkg, &path).await {
                                println!("Failed to download package: {}", e);
                            }
                        }
                    }
                    Err(e) => println!("Fail to list package versions: {:#?}", e),
                }
            }
        }
    }
}
