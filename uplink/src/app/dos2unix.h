#if !defined __DOS2UNIX_H

#include <iostream>
#include <fstream>


class dos2unixbuf : public std::streambuf {
public:
#ifdef WIN32
	typedef std::ios::open_mode openmode;
#else
	typedef std::ios::openmode openmode;
#endif

  dos2unixbuf(const char *filename, openmode mode);
  virtual ~dos2unixbuf();

  void close();

protected:
  virtual int overflow(int c = EOF);
  virtual int underflow();
  virtual int uflow();
  virtual int pbackfail(int c = EOF);
  virtual int sync();
  
private:
  std::filebuf inner;
  char *buffer;
};

class idos2unixstream
  : public std::istream {
public:
  idos2unixstream(const char *filename)
    : std::istream(new dos2unixbuf(filename, std::ios::in))
    {
    }

  virtual ~idos2unixstream();

  void close();
};

#endif // __DOS2UNIX_H
