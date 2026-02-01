#include "utils.h"

int main(int argc, char **argv);

uint16_t strlength(char *ch) {
  uint16_t i = 0; // Changed counter to 0
  while (ch[i++])
    ;
  return i - 1; // Changed counter to i instead of i--
}

int atoi(char *str) {
  int res = 0;

  for (int i = 0; str[i] != '\0'; ++i)
    res = res * 10 + str[i] - '0';

  return res;
}

void reverse(char s[]) {
  int  i, j;
  char c;

  for (i = 0, j = strlength(s) - 1; i < j; i++, j--) {
    c = s[i];
    s[i] = s[j];
    s[j] = c;
  }
}

void itoa(int n, char s[]) {
  int i, sign;

  if ((sign = n) < 
