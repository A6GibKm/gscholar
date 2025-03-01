use std::fmt;

extern crate reqwest;
extern crate select;

use scraper::{Html, Selector};

pub struct Client {
    client: reqwest::Client,
}

#[derive(Debug)]
pub enum Error {
    ConnectionError(String),
    ParseError,
    InvalidServiceError,
    RequiredFieldError,
    NotImplementedError,
    InvalidResponseError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionError(url) => write!(f, "Could not connect to {url}"),
            _ => write!(f, "{:?}", self),
        }
    }
}
impl std::error::Error for Error {}

pub struct ScholarResult {
    pub title: String,
    pub author: String,
    pub abs: String,
    pub link: String,
}

pub struct ScholarArgs {
    // q - required
    pub query: String,

    // cites - citaction id to trigger "cited by"
    pub cite_id: Option<&'static str>,

    // as_ylo - give results from this year onwards
    pub from_year: Option<u16>,

    // as_yhi
    pub to_year: Option<u16>,

    // scisbd - 0 for relevence, 1 to include only abstracts, 2 for everything. Default = date
    pub sort_by: Option<u8>,

    // cluster - query all versions. Use with q and cites prohibited
    pub cluster_id: Option<&'static str>,

    // hl - eg: hl=en for english
    pub lang: Option<&'static str>,

    // lr - one or multiple languages to limit the results to
    // eg: lr=lang_fr|lang_en
    pub lang_limit: Option<&'static str>,

    // num - max number of results to return
    pub limit: Option<u32>,

    // start - result offset. Can be used with limit for pagination
    pub offset: Option<u32>,

    // safe - level of filtering
    // safe=active or safe=off
    pub adult_filtering: Option<bool>,

    // filter - whether to give similar/ommitted results
    // filter=1 for similar results and 0 for ommitted
    pub include_similar_results: Option<bool>,

    // as_vis - set to 1 for including citations, otherwise 0
    pub include_citations: Option<bool>,
}

impl ScholarArgs {
    fn get_service(&self) -> Services {
        Services::Scholar
    }

    pub fn get_url(&self) -> Result<String, Error> {
        let mut url = String::from(get_base_url(self.get_service()));

        if self.query.is_empty() {
            return Err(Error::RequiredFieldError);
        }

        url.push_str("q=");
        url.push_str(&self.query);

        if let Some(i) = self.cite_id {
            url.push_str("&cites=");
            url.push_str(i);
        }
        if let Some(i) = self.from_year {
            url.push_str("&as_ylo=");
            url.push_str(&i.to_string()[..]);
        }
        if let Some(i) = self.to_year {
            url.push_str("&as_yhi=");
            url.push_str(&i.to_string()[..]);
        }
        if let Some(i) = self.sort_by {
            if i < 3 {
                url.push_str("&scisbd=");
                url.push_str(&i.to_string()[..]);
            }
        }
        if let Some(i) = self.cluster_id {
            url.push_str("&cluster=");
            url.push_str(i);
        }
        if let Some(i) = self.lang {
            // TODO: validation
            url.push_str("&hl=");
            url.push_str(i);
        }
        if let Some(i) = self.lang_limit {
            // TODO: validation
            url.push_str("&lr=");
            url.push_str(i);
        }
        if let Some(i) = self.limit {
            url.push_str("&num=");
            url.push_str(&i.to_string()[..]);
        }
        if let Some(i) = self.offset {
            url.push_str("&start=");
            url.push_str(&i.to_string()[..]);
        }
        if let Some(i) = self.adult_filtering {
            url.push_str("&safe=");
            if i {
                url.push_str("active");
            } else {
                url.push_str("off");
            }
        }
        if let Some(i) = self.include_similar_results {
            url.push_str("&filter=");
            if i {
                url.push('1');
            } else {
                url.push('0');
            }
        }
        if let Some(i) = self.include_citations {
            url.push_str("&as_vis=");
            if i {
                url.push('1');
            } else {
                url.push('0');
            }
        }
        Ok(url::Url::parse(&url).map_err(|_| Error::ParseError)?.to_string())
    }
}

pub enum Services {
    Scholar,
}

pub fn init_client() -> Client {
    let client = reqwest::Client::new();
    Client { client }
}

fn get_base_url<'a>(service: Services) -> &'a str {
    match service {
        Services::Scholar => "https://scholar.google.com/scholar?",
    }
}

impl Client {
    async fn get_document(&self, url: &str) -> Result<String, Error> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_err| Error::ConnectionError(url.to_string()))?;
        let val: String = resp.text().await.map_err(|_| Error::ParseError)?;
        Ok(val)
    }

    fn scrape_serialize(&self, document: String) -> Result<Vec<ScholarResult>, Error> {
        let fragment = Html::parse_document(&document[..]);

        let article_selector = Selector::parse(".gs_ri").map_err(|_| Error::ParseError)?;
        let title_selector = Selector::parse(".gs_rt").map_err(|_| Error::ParseError)?;
        let abstract_selector = Selector::parse(".gs_rs").map_err(|_| Error::ParseError)?;
        let author_selector = Selector::parse(".gs_a").map_err(|_| Error::ParseError)?;
        let link_selector = Selector::parse("a").map_err(|_| Error::ParseError)?;

        let nodes = fragment.select(&article_selector).collect::<Vec<_>>();

        let response = nodes
            .chunks_exact(1)
            .filter_map(|rows| {
                let title = rows.get(0)?.select(&title_selector).next()?;
                let link = rows
                    .get(0)?
                    .select(&link_selector)
                    .next()
                    .and_then(|n| n.value().attr("href"))?;
                let abs = rows.get(0)?.select(&abstract_selector).next()?;
                let author = rows.get(0)?.select(&author_selector).next()?;

                let ti = title.text().collect::<String>();
                let ab = abs.text().collect::<String>();
                let au = author.text().collect::<String>();
                let li = link.to_string();

                let result = ScholarResult {
                    title: ti,
                    author: au,
                    abs: ab,
                    link: li,
                };
                Some(result)
            })
            .collect::<Vec<ScholarResult>>();

        Ok(response)
    }

    pub async fn scrape_scholar(&self, args: &ScholarArgs) -> Result<Vec<ScholarResult>, Error> {
        let url = args.get_url()?;
        let doc = self.get_document(&url).await?;

        self.scrape_serialize(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_url_query() {
        let sc = ScholarArgs {
            query: "abcd",
            cite_id: None,
            from_year: None,
            to_year: None,
            sort_by: None,
            cluster_id: None,
            lang: None,
            lang_limit: None,
            limit: None,
            offset: None,
            adult_filtering: None,
            include_similar_results: None,
            include_citations: None,
        };

        match sc.get_url() {
            Ok(url) => assert!(
                url.eq("https://scholar.google.com/scholar?q=abcd"),
                "value was {}",
                url
            ),
            Err(_e) => assert_eq!(false, true),
        }
    }

    #[test]
    fn build_url_all() {
        let sc = ScholarArgs {
            query: "abcd",
            cite_id: Some("213123123123"),
            from_year: Some(2018),
            to_year: Some(2021),
            sort_by: Some(0),
            cluster_id: Some("3121312312"),
            lang: Some("en"),
            lang_limit: Some("lang_fr|lang_en"),
            limit: Some(10),
            offset: Some(5),
            adult_filtering: Some(true),
            include_similar_results: Some(true),
            include_citations: Some(true),
        };
        match sc.get_url() {
            Ok(url) => assert!(
                url.eq("https://scholar.google.com/scholar?q=abcd&cites=213123123123&as_ylo=2018&as_yhi=2021&scisbd=0&cluster=3121312312&hl=en&lr=lang_fr|lang_en&num=10&start=5&safe=active&filter=1&as_vis=1"), "value was {}", url),
            Err(_e) => assert_eq!(false, true),
        }
    }

    #[tokio::test]
    async fn scrape_with_query() {
        let sc = ScholarArgs {
            query: "machine-learning",
            cite_id: None,
            from_year: None,
            to_year: None,
            sort_by: None,
            cluster_id: None,
            lang: None,
            lang_limit: None,
            limit: Some(3),
            offset: Some(0),
            adult_filtering: None,
            include_similar_results: None,
            include_citations: None,
        };
        match sc.get_url() {
            Ok(url) => println!("_URLS {}", url),
            Err(_e) => assert_eq!(false, true),
        }

        let client = init_client();
        match client.scrape_scholar(&sc).await {
            Ok(res) => assert_eq!(res.len(), 3),
            Err(_e) => assert_eq!(true, false),
        }
    }
}
