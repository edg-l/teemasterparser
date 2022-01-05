from datetime import timedelta, date, datetime

first_day = date(2021, 5, 18)
now = datetime.now().date()

print("# Player count statistics")

for single_date in (first_day + timedelta(n) for n in range((now - first_day).days - 1)):
    print(f"## {single_date}")
    print(f"![{single_date} data](images/{single_date}.svg)")