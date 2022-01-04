# teemasterparser

Parses the data from one day in https://ddnet.tw/stats/master/ to a gnuplot format.

# example
```
wget https://ddnet.tw/stats/master/2022-01-03.tar.zstd

tar --use-compress-program=unzstd -xvf 2022-01-03.tar.zstd

teemasterparser -d 2022-01-03/

# check image.svg
```
