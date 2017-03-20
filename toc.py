import fileinput, re

defaults = {'filter': 'LLKP.filter',
            'regex_match': r'# \d\d\d ',
            'replacement': '# {} ',
            'pivot': 'SETTINGS',
            'index_start': 0,
            'digits': 3}

def regex(filter=defaults['filter'],
          regex_match=defaults['regex_match'],
          replacement=defaults['replacement'],
          pivot=defaults['pivot'],
          index_start=defaults['index_start'],
          digits=defaults['digits']):
    with fileinput.FileInput(filter, inplace=True) as text:
        pattern = re.compile(regex_match)
        restart = False
        index = index_start
        for line in text:
            if not restart and pivot in line:
                index = index_start
                restart = True
            if pattern.match(line):
                index_str = str(index).zfill(digits)
                line = pattern.sub(replacement.format(index_str), line)
                index += 1
            print(line, end='')

if __name__ == '__main__':
    regex()
