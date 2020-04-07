Test
---

    import requests

    proxies = dict(http='socks5://127.0.0.1:8088')
    resp = requests.get(
        'http://www.baidu.com',
        proxies=proxies
        )